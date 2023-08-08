use tree_sitter::TreeCursor;

use crate::{
    config::CONFIG,
    cst::*,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{create_alias, ensure_kind, ComplementKind, Visitor, COMMENT},
};

impl Visitor {
    /// エイリアス可能な式
    /// 呼び出し後、cursorはaliasまたは式のノードを指している
    pub(crate) fn visit_aliasable_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        // 補完する場合の補完の種類
        complement_kind: Option<&ComplementKind>,
        // ASキーワードを補完/省略するかどうか
        complement_as: bool,
        // エイリアスを補完するかどうか
        complement_alias: bool,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        let complement_as = complement_as && CONFIG.read().unwrap().complement_as_keyword;
        let complement_alias = complement_alias && CONFIG.read().unwrap().complement_alias;

        // エイリアス可能な式の定義
        //    _aliasable_expression =
        //        alias | _expression

        //    alias =
        //        _expression
        //        ["AS"]
        //        identifier

        let comment = if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        match cursor.node().kind() {
            "alias" => {
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let mut lhs_expr = self.visit_expr(cursor, src)?;
                if let Some(comment) = comment {
                    if comment.loc().is_next_to(&lhs_expr.loc()) {
                        lhs_expr.set_head_comment(comment);
                    } else {
                        // エイリアス式の直前のコメントは、バインドパラメータしか考慮していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_aliasable_expr(): unexpected comment\n{:?}",
                            cursor.node().range()
                        )));
                    }
                }

                let mut aligned = AlignedExpr::new(lhs_expr);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() {
                    // cursor -> trailing_comment | "AS"?

                    if cursor.node().kind() == COMMENT {
                        // ASの直前にcommentがある場合
                        let comment = Comment::new(cursor.node(), src);

                        if comment.is_block_comment() || !comment.loc().is_same_line(&aligned.loc())
                        {
                            // 行末以外のコメント(次以降の行のコメント)は未定義
                            // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                            // エイリアスがない場合は、コメントノードがここに現れない
                            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                "visit_aliasable_expr(): unexpected syntax\nnode_kind: {}\n{:#?}",
                                cursor.node().kind(),
                                cursor.node().range(),
                            )));
                        } else {
                            // 行末コメント
                            aligned.set_lhs_trailing_comment(comment)?;
                        }
                        cursor.goto_next_sibling();
                    }

                    let as_keyword = if cursor.node().kind() == "AS" {
                        let keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        cursor.goto_next_sibling();

                        Some(keyword)
                    } else {
                        None
                    };

                    let as_keyword = match (complement_kind, as_keyword) {
                        (Some(ComplementKind::TableName), Some(_)) if complement_as => {
                            // テーブル名ルールを適用する、かつAS補完ONの場合、ASを省略
                            None
                        }
                        (Some(ComplementKind::ColumnName), None) if complement_as => {
                            // カラム名ルールを適用する、かつAS補完ONの場合、ASを補完
                            Some(convert_keyword_case("AS"))
                        }
                        (_, Some(as_keyword)) => Some(convert_keyword_case(as_keyword)),
                        _ => None,
                    };

                    //右辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    ensure_kind(cursor, "identifier")?;

                    let rhs_expr =
                        PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Expr);
                    aligned.add_rhs(as_keyword, Expr::Primary(Box::new(rhs_expr)));
                }

                // cursorをalias に戻す
                cursor.goto_parent();

                Ok(aligned)
            }
            _ => {
                // _expression
                let mut expr = self.visit_expr(cursor, src)?;

                if let Some(comment) = comment {
                    expr.set_head_comment(comment);
                }

                let mut aligned = AlignedExpr::new(expr.clone());

                if complement_alias {
                    // エイリアス名を生成できた場合に、エイリアス補完を行う
                    if let Some(alias_name) = create_alias(&expr) {
                        match complement_kind {
                            Some(ComplementKind::TableName) => {
                                // テーブル名ルールでエイリアス補完
                                aligned.add_rhs(None, alias_name);
                            }
                            Some(ComplementKind::ColumnName) => {
                                // カラム名ルールでエイリアス補完
                                aligned.add_rhs(Some(convert_keyword_case("AS")), alias_name);
                            }
                            _ => {}
                        }
                    }
                }

                Ok(aligned)
            }
        }
    }
}
