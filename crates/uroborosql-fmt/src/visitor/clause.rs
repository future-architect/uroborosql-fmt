mod for_locking;
mod frame;
mod from;
mod group;
mod having;
mod limit;
mod offset;
mod returning;
mod select;
mod set;
mod sort;
mod values;
mod where_clause;
mod where_or_current;
mod with;

use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{
        AlignedExpr, AsteriskExpr, Body, Comment, Expr, PrimaryExpr, PrimaryExprKind,
        SeparatedLines,
    },
    error::UroboroSQLFmtError,
    util::{convert_identifier_case, convert_keyword_case},
    visitor::{create_alias_from_expr, pg_ensure_kind, Visitor, COMMA},
    CONFIG,
};

use super::pg_error_annotation_from_cursor;

impl Visitor {
    pub(crate) fn visit_qualified_name(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<PrimaryExpr, UroboroSQLFmtError> {
        // qualified_name
        // - ColId
        // - ColId indirection

        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::ColId, src);

        let mut qualified_name_text = cursor.node().text().to_string();

        if cursor.goto_next_sibling() {
            // indirection が存在する場合
            pg_ensure_kind!(cursor, SyntaxKind::indirection, src);

            let indirection_text = cursor.node().text().to_string();

            if indirection_text.contains('[') {
                // この場所での subscript （[1] など）は構文定義上可能だが、PostgreSQL側でrejectされる不正な記述
                // - https://github.com/postgres/postgres/blob/e2809e3a1015697832ee4d37b75ba1cd0caac0f0/src/backend/parser/gram.y#L17317-L17323
                // - https://github.com/postgres/postgres/blob/e2809e3a1015697832ee4d37b75ba1cd0caac0f0/src/backend/parser/gram.y#L18963-L18967
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_qualified_name(): invalid subscript notation appeared.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            // 空白を除去してqualified_name_textに追加
            qualified_name_text.push_str(
                &indirection_text
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>(),
            );
        }

        cursor.goto_parent();
        // cursor -> qualified_name
        pg_ensure_kind!(cursor, SyntaxKind::qualified_name, src);

        let primary = PrimaryExpr::new(
            convert_identifier_case(&qualified_name_text),
            cursor.node().range().into(),
        );

        Ok(primary)
    }

    /// target_list をフォーマットする
    /// 直前にカンマがある場合は extra_leading_comma として渡す
    pub(crate) fn visit_target_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
        extra_leading_comma: Option<String>,
    ) -> Result<Body, UroboroSQLFmtError> {
        // target_list -> target_el ("," target_el)*

        // target_listは必ずtarget_elを子供に持つ
        cursor.goto_first_child();

        // cursor -> target_el
        let mut sep_lines = SeparatedLines::new();

        let target_el = self.visit_target_el(cursor, src)?;
        sep_lines.add_expr(target_el, extra_leading_comma, vec![]);

        while cursor.goto_next_sibling() {
            // cursor -> "," または target_el
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::target_el => {
                    let target_el = self.visit_target_el(cursor, src)?;
                    sep_lines.add_expr(target_el, Some(COMMA.to_string()), vec![]);
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    sep_lines.add_comment_to_child(comment)?;
                }
                SyntaxKind::C_COMMENT => {
                    let comment_node = cursor.node();
                    let comment = Comment::pg_new(comment_node);

                    // バインドパラメータ判定のためにコメントの次のノードを取得する
                    let Some(next_sibling) = cursor.node().next_sibling() else {
                        // 最後の要素の行末にあるコメントは、 target_list の直下に現れず target_list と同階層の要素になる
                        // そのためコメントが最後の子供になることはなく、次のノードを必ず取得できる

                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_target_list(): unexpected node kind\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    };

                    // コメントノードがバインドパラメータであるかを判定
                    // バインドパラメータならば式として処理し、そうでなければコメントとして処理する
                    if comment.loc().is_next_to(&next_sibling.range().into())
                        && next_sibling.kind() == SyntaxKind::target_el
                    {
                        cursor.goto_next_sibling();

                        let mut target_el = self.visit_target_el(cursor, src)?;
                        target_el.set_head_comment(comment);

                        sep_lines.add_expr(target_el, Some(COMMA.to_string()), vec![]);
                    } else {
                        sep_lines.add_comment_to_child(comment)?;
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_target_list(): unexpected node kind\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // cursorをtarget_listに
        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::target_list, src);

        Ok(Body::SepLines(sep_lines))
    }

    fn visit_target_el(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // target_el
        // - a_expr
        // - Star
        // - a_expr AS ColLabel
        // - a_expr BareColLabel

        cursor.goto_first_child();

        // a_expr -> c_expr -> columnref という構造になっているかどうか
        // 識別子の場合は columnref ノードになるため、識別子判定を columnref ノードか否かで行っている
        let is_columnref = match cursor.node().kind() {
            SyntaxKind::a_expr => {
                cursor.goto_first_child();
                let is_col = match cursor.node().kind() {
                    SyntaxKind::c_expr => {
                        cursor.goto_first_child();
                        let is_col = cursor.node().kind() == SyntaxKind::columnref;
                        cursor.goto_parent();
                        is_col
                    }
                    _ => false,
                };
                cursor.goto_parent();
                is_col
            }
            _ => false,
        };

        let lhs_expr = match cursor.node().kind() {
            SyntaxKind::a_expr => self.visit_a_expr_or_b_expr(cursor, src)?,
            SyntaxKind::Star => {
                // Star は postgresql-cst-parser の語彙で、uroborosql-fmt::cst では AsteriskExpr として扱う
                // Star は postgres の文法上 Expression ではないが、 cst モジュールの Expr に変換する
                let asterisk =
                    AsteriskExpr::new(cursor.node().text(), cursor.node().range().into());

                Expr::Asterisk(Box::new(asterisk))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_target_el(): excepted node is {}, but actual {}\n{}",
                    SyntaxKind::target_el,
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        let mut aligned = AlignedExpr::new(lhs_expr.clone());

        // (`AS` ColLabel` | `BareColLabel`)?
        if cursor.goto_next_sibling() {
            // cursor -> comment
            // AS の直前にコメントがある場合
            if cursor.node().is_comment() {
                let comment = Comment::pg_new(cursor.node());

                if comment.is_block_comment() || !comment.loc().is_same_line(&lhs_expr.loc()) {
                    // 行末以外のコメント(次以降の行のコメント)は未定義
                    // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_target_el(): unexpected comment\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                } else {
                    // 行末コメント
                    aligned.set_lhs_trailing_comment(comment)?;
                }
                cursor.goto_next_sibling();
            }

            let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
                let keyword = cursor.node().text();
                cursor.goto_next_sibling();

                Some(convert_keyword_case(keyword))
            } else {
                // ASキーワードが存在しない場合
                if CONFIG.read().unwrap().complement_column_as_keyword {
                    Some(convert_keyword_case("AS"))
                } else {
                    None
                }
            };

            cursor.goto_next_sibling();
            // cursor -> ColLabel | BareColLabel
            // ColLabel は as キーワードを使ったときのラベルで、BareColLabel は as キーワードを使わないときのラベル
            // ColLabel にはどんなラベルも利用でき、 BareColLabel の場合は限られたラベルしか利用できないという構文上の区別があるが、
            // フォーマッタとしては関係がないので同じように扱う
            pg_ensure_kind!(cursor, SyntaxKind::ColLabel | SyntaxKind::BareColLabel, src);
            let rhs_expr = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
            aligned.add_rhs(as_keyword, rhs_expr.into());
        } else {
            // エイリアスを補完する設定が有効で、かつ識別子の場合はエイリアスを補完する
            if CONFIG.read().unwrap().complement_alias && is_columnref {
                if let Some(alias_name) = create_alias_from_expr(&lhs_expr) {
                    aligned.add_rhs(Some(convert_keyword_case("AS")), alias_name);
                }
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::target_el, src);

        Ok(aligned)
    }
}
