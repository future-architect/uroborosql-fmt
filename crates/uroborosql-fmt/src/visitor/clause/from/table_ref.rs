mod func_table;
mod joined_table;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{from_list::TableRef, *},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
    CONFIG,
};

impl Visitor {
    /// 呼出し後、cursor は table_ref を指している
    pub(crate) fn visit_table_ref(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<TableRef, UroboroSQLFmtError> {
        // table_ref
        // - relation_expr opt_alias_clause [tablesample_clause]
        // - select_with_parens opt_alias_clause
        // - joined_table
        // - '(' joined_table ')' alias_clause
        // - func_table func_alias_clause
        // - xmltable opt_alias_clause
        // - json_table opt_alias_clause
        // - LATERAL func_table func_alias_clause
        // - LATERAL xmltable opt_alias_clause
        // - LATERAL select_with_parens opt_alias_clause
        // - LATERAL json_table opt_alias_clause

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::relation_expr => {
                // 通常のテーブル参照
                // relation_expr opt_alias_clause [tablesample_clause]

                let table_name = self.visit_relation_expr(cursor, src)?;
                let mut table_ref = table_name.to_aligned();

                cursor.goto_next_sibling();

                // cursor -> comment?
                // エイリアスの直前にコメントが来る場合
                if cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());

                    // 行末以外のコメント（次行以降のコメント）は未定義
                    // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                    if !comment.is_block_comment() && comment.loc().is_same_line(&table_ref.loc()) {
                        table_ref.set_lhs_trailing_comment(comment)?;
                    } else {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_table_ref(): unexpected comment\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    cursor.goto_next_sibling();
                }

                // cursor -> opt_alias_clause?
                if cursor.node().kind() == SyntaxKind::opt_alias_clause {
                    // opt_alias_clause
                    // - alias_clause
                    cursor.goto_first_child();
                    let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

                    // AS補完
                    if let Some(as_keyword) = as_keyword {
                        // AS があり、かつ AS を除去する設定が有効ならば AS を除去する
                        if CONFIG.read().unwrap().remove_table_as_keyword {
                            table_ref.add_rhs(None, col_id);
                        } else {
                            table_ref.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                        }
                    } else {
                        // ASが無い場合は補完しない
                        table_ref.add_rhs(None, col_id);
                    }

                    cursor.goto_parent();
                    ensure_kind!(cursor, SyntaxKind::opt_alias_clause, src);
                    cursor.goto_next_sibling();
                }

                // cursor -> tablesample_clause?
                if cursor.node().kind() == SyntaxKind::tablesample_clause {
                    // TABLESAMPLE
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_table_ref(): tablesample_clause node appeared. Tablesample is not implemented yet.\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                cursor.goto_parent();
                // cursor -> table_ref
                ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(table_ref))
            }
            SyntaxKind::select_with_parens => {
                // サブクエリ
                // select_with_parens opt_alias_clause

                let sub_query = self.visit_select_with_parens(cursor, src)?;
                let mut table_ref = sub_query.to_aligned();

                cursor.goto_next_sibling();

                // cursor -> comment?
                // エイリアスの直前にコメントが来る場合
                if cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());

                    // 行末以外のコメント（次行以降のコメント）は未定義
                    // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                    if !comment.is_block_comment() && comment.loc().is_same_line(&table_ref.loc()) {
                        table_ref.set_lhs_trailing_comment(comment)?;
                    } else {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_table_ref(): unexpected comment\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    cursor.goto_next_sibling();
                }

                // cursor -> opt_alias_clause?
                if cursor.node().kind() == SyntaxKind::opt_alias_clause {
                    // opt_alias_clause
                    // - alias_clause
                    cursor.goto_first_child();

                    let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

                    if let Some(as_keyword) = as_keyword {
                        // AS があり、かつ AS を除去する設定が有効ならば AS を除去する
                        if CONFIG.read().unwrap().remove_table_as_keyword {
                            table_ref.add_rhs(None, col_id);
                        } else {
                            table_ref.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                        }
                    } else {
                        // ASが無い場合は補完しない
                        table_ref.add_rhs(None, col_id);
                    }

                    cursor.goto_parent();
                    ensure_kind!(cursor, SyntaxKind::opt_alias_clause, src);
                }

                cursor.goto_parent();
                // cursor -> table_ref
                ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(table_ref))
            }
            SyntaxKind::joined_table => {
                // テーブル結合
                let joined_table = self.visit_joined_table(cursor, src)?;

                cursor.goto_parent();
                ensure_kind!(cursor, SyntaxKind::table_ref, src);

                // joined_tableがExpr::JoinedTableの場合はTableRef::JoinedTableに、
                // それ以外（括弧付きなど）はTableRef::SimpleTableに変換
                match joined_table {
                    Expr::JoinedTable(joined_table_box) => {
                        Ok(TableRef::JoinedTable(joined_table_box))
                    }
                    other => Ok(TableRef::SimpleTable(other.to_aligned())),
                }
            }
            SyntaxKind::LParen => {
                // 括弧付き結合
                // '(' joined_table ')' alias_clause

                cursor.goto_next_sibling();

                let joined_table = self.visit_joined_table(cursor, src)?;
                // ParenExpr を作成
                let parenthesized_joined_table =
                    ParenExpr::new(joined_table, Location::from(cursor.node().range()));

                let mut paren = Expr::ParenExpr(Box::new(parenthesized_joined_table));

                cursor.goto_next_sibling();
                // cursor -> comment?
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    paren.add_comment_to_child(comment)?;
                    cursor.goto_next_sibling();
                }

                ensure_kind!(cursor, SyntaxKind::RParen, src);

                let mut aligned = paren.to_aligned();

                cursor.goto_next_sibling();
                ensure_kind!(cursor, SyntaxKind::alias_clause, src);
                let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

                // as の補完はしない。as が存在し、 remove_table_as_keyword が有効ならば AS を除去
                if let Some(as_keyword) = as_keyword {
                    if CONFIG.read().unwrap().remove_table_as_keyword {
                        aligned.add_rhs(None, col_id);
                    } else {
                        aligned.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                    }
                } else {
                    aligned.add_rhs(None, col_id);
                }

                cursor.goto_parent();
                ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(aligned))
            }
            SyntaxKind::func_table => {
                // テーブル関数呼び出し
                // func_table func_alias_clause

                // cursor -> func_table
                let func_table = self.visit_func_table(cursor, src)?;
                let func_table_expr = Expr::FunctionTable(Box::new(func_table));
                let mut aligned = func_table_expr.to_aligned();

                cursor.goto_next_sibling();
                // cursor -> func_alias_clause
                ensure_kind!(cursor, SyntaxKind::func_alias_clause, src);
                let (as_keyword, alias_expr) = self.visit_func_alias_clause(cursor, src)?;
                aligned.add_rhs(as_keyword, alias_expr);

                cursor.goto_parent();
                // cursor -> table_ref
                ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(aligned))
            }
            SyntaxKind::LATERAL_P => {
                // LATERAL系
                // LATERAL func_table func_alias_clause
                // LATERAL xmltable opt_alias_clause
                // LATERAL select_with_parens opt_alias_clause
                // LATERAL json_table opt_alias_clause
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): LATERAL_P node appeared. LATERAL expressions are not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::xmltable => {
                // XMLテーブル
                // xmltable opt_alias_clause
                // - XMLTABLE('/root/row' PASSING data COLUMNS id int PATH '@id')
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): xmltable node appeared. XML tables are not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                // TODO: json_table
                Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_table_ref(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
        }
    }
}
