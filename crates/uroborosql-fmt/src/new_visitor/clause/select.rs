use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{select::SelectBody, *},
    error::UroboroSQLFmtError,
    new_visitor::{
        create_alias_from_expr, pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor,
        Visitor, COMMA,
    },
    util::convert_keyword_case,
    CONFIG,
};

/// 補完の種類
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ComplementKind {
    /// テーブル名
    TableName,
    /// カラム名
    ColumnName,
}

/// AS補完/除去、エイリアス補完に関する設定
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComplementConfig {
    /// 補完の種類
    kind: ComplementKind,
    /// kindに合わせてASキーワードを補完/除去するかどうか
    complement_or_remove_as: bool,
    /// kindに合わせてエイリアスを補完するかどうか
    complement_alias: bool,
}

impl Default for ComplementConfig {
    fn default() -> Self {
        ComplementConfig {
            // デフォルト値としてTableNameを設定しているが、デフォルトでは補完しないのでTableNameとColumnNameのどちらを設定していても変わらない
            kind: ComplementKind::TableName,
            complement_or_remove_as: false,
            complement_alias: false,
        }
    }
}

impl ComplementConfig {
    pub(crate) fn new(
        kind: ComplementKind,
        complement_or_remove_as: bool,
        complement_alias: bool,
    ) -> ComplementConfig {
        ComplementConfig {
            kind,
            complement_or_remove_as,
            complement_alias,
        }
    }

    /// 自身の設定と定義ファイルの設定を考慮してASを補完をすべきかどうか返す
    fn complement_as_keyword(&self) -> bool {
        self.complement_or_remove_as
            && CONFIG.read().unwrap().complement_column_as_keyword
            && self.kind == ComplementKind::ColumnName
    }

    /// 自身の設定と定義ファイルの設定を考慮してASを削除をすべきかどうか返す
    fn remove_as_keyword(&self) -> bool {
        self.complement_or_remove_as
            && CONFIG.read().unwrap().remove_table_as_keyword
            && self.kind == ComplementKind::TableName
    }

    /// 自身の設定と定義ファイルの設定を考慮してエイリアスを補完すべきかどうか返す
    pub(crate) fn complement_alias(&self) -> bool {
        self.complement_alias
            && CONFIG.read().unwrap().complement_alias
            && self.kind == ComplementKind::ColumnName
    }
}

impl Visitor {
    /// SELECT句
    /// 呼び出し後、cursorはselect_clauseを指している
    pub(crate) fn visit_select_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // SELECT句の定義
        //      select_clause =
        //          SELECT
        //          [ ALL | DISTINCT [ ON ( expression [, ...] ) ] ] ]
        //          [ select_clause_body ]

        pg_ensure_kind(cursor, SyntaxKind::SELECT, src)?;

        // cursor -> SELECT
        let mut clause = pg_create_clause(cursor, SyntaxKind::SELECT)?;
        cursor.goto_next_sibling();

        // SQL_IDとコメントを消費
        self.pg_consume_or_complement_sql_id(cursor, &mut clause);
        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        let mut select_body = SelectBody::new();

        // TODO: all, distinct
        // // [ ALL | DISTINCT [ ON ( expression [, ...] ) ] ] ]
        // match cursor.node().kind() {
        //     "ALL" => {
        //         let all_clause = create_clause(cursor, src, "ALL")?;

        //         select_body.set_all_distinct(all_clause);

        //         cursor.goto_next_sibling();
        //     }
        //     "DISTINCT" => {
        //         let mut distinct_clause = create_clause(cursor, src, "DISTINCT")?;

        //         cursor.goto_next_sibling();

        //         // ON ( expression [, ...] )
        //         if cursor.node().kind() == "ON" {
        //             // DISTINCTにONキーワードを追加
        //             distinct_clause.extend_kw(cursor.node(), src);

        //             cursor.goto_next_sibling();

        //             // ( expression [, ...] ) をColumnList構造体に格納
        //             let mut column_list = self.visit_column_list(cursor, src)?;
        //             // 改行によるフォーマットを強制
        //             column_list.set_force_multi_line(true);

        //             // ColumntListをSeparatedLinesに格納してBody
        //             let mut sep_lines = SeparatedLines::new();

        //             sep_lines.add_expr(
        //                 Expr::ColumnList(Box::new(column_list)).to_aligned(),
        //                 None,
        //                 vec![],
        //             );

        //             distinct_clause.set_body(Body::SepLines(sep_lines));
        //         }

        //         select_body.set_all_distinct(distinct_clause);

        //         cursor.goto_next_sibling();
        //     }
        //     _ => {}
        // }

        // cursor -> target_list
        if cursor.node().kind() == SyntaxKind::target_list {
            let target_list = self.visit_target_list(cursor, src)?;
            // select_clause_body 部分に target_list から生成した Body をセット
            select_body.set_select_clause_body(target_list);
        }

        clause.set_body(Body::Select(Box::new(select_body)));

        // cursor.goto_parent(); // SelectStmt goto parent しちゃだめ
        // pg_ensure_kind(cursor, SyntaxKind::SelectStmt, src)?;

        cursor.goto_next_sibling(); // select の次

        Ok(clause)
    }

    /// [pg] postgresql-cst-parser の target_list を Body::SeparatedLines に変換する
    /// tree-sitter の select_clause_body が該当
    /// 呼び出し後、cursorは target_list を指している
    fn visit_target_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // target_list -> target_el ("," target_el)*

        // target_listは必ずtarget_elを子供に持つ
        cursor.goto_first_child();

        // cursor -> target_el
        let mut sep_lines = SeparatedLines::new();

        let complement_config = ComplementConfig::new(ComplementKind::ColumnName, true, true);
        let target_el = self.visit_target_el(cursor, src, &complement_config)?;
        sep_lines.add_expr(target_el, None, vec![]);

        while cursor.goto_next_sibling() {
            // cursor -> "," または target_el
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::target_el => {
                    let target_el = self.visit_target_el(cursor, src, &complement_config)?;
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

                        let mut target_el =
                            self.visit_target_el(cursor, src, &complement_config)?;
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
        pg_ensure_kind(cursor, SyntaxKind::target_list, &src)?;

        Ok(Body::SepLines(sep_lines))
    }

    fn visit_target_el(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
        complement_config: &ComplementConfig,
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
            SyntaxKind::a_expr => self.visit_a_expr(cursor, src)?,
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

        // (`AS` ColLabel` | `BareColLabel`)?
        let aligned = if cursor.goto_next_sibling() {
            let mut aligned = AlignedExpr::new(lhs_expr);

            let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
                let keyword = cursor.node().text();
                cursor.goto_next_sibling();

                // ASキーワードが存在する場合
                if complement_config.remove_as_keyword() {
                    None
                } else {
                    Some(convert_keyword_case(keyword))
                }
            } else {
                // ASキーワードが存在しない場合
                if complement_config.complement_as_keyword() {
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
            let rhs_expr = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
            aligned.add_rhs(as_keyword, rhs_expr.into());

            aligned
        } else {
            // エイリアスがない場合
            let mut aligned = AlignedExpr::new(lhs_expr.clone());

            // エイリアス補完オプションが有効であり、カラム参照である場合にエイリアス補完を行う
            if complement_config.complement_alias() && is_columnref {
                if let Some(alias_name) = create_alias_from_expr(&lhs_expr) {
                    aligned.add_rhs(Some(convert_keyword_case("AS")), alias_name);
                }
            }

            aligned
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::target_el, src)?;

        Ok(aligned)
    }
}
