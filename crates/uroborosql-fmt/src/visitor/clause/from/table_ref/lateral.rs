use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{from_list::TableRef, *},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
    CONFIG,
};

impl Visitor {
    /// LATERAL系のtable_refを処理する
    /// LATERAL func_table func_alias_clause
    /// LATERAL xmltable opt_alias_clause
    /// LATERAL select_with_parens opt_alias_clause
    /// LATERAL json_table opt_alias_clause
    ///
    /// 呼出し時、cursorはLATERAL_Pを指している
    /// 呼出し後、cursorはtable_refを指している
    pub(crate) fn visit_lateral_table_ref(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<TableRef, UroboroSQLFmtError> {
        // LATERALキーワードを取得
        ensure_kind!(cursor, SyntaxKind::LATERAL_P, src);
        let lateral_keyword = convert_keyword_case(cursor.node().text());
        let lateral_loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();

        // コメントをスキップ（LATERALとサブクエリの間のコメント）
        // 現状、LATERAL と サブクエリの間のコメントは未対応
        while cursor.node().is_comment() {
            cursor.goto_next_sibling();
        }

        match cursor.node().kind() {
            SyntaxKind::select_with_parens => {
                // LATERAL select_with_parens opt_alias_clause
                self.visit_lateral_subquery(cursor, src, lateral_keyword, lateral_loc)
            }
            SyntaxKind::func_table => {
                // LATERAL func_table func_alias_clause
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_lateral_table_ref(): LATERAL func_table is not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::xmltable => {
                // LATERAL xmltable opt_alias_clause
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_lateral_table_ref(): LATERAL xmltable is not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                // LATERAL json_table など
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_lateral_table_ref(): LATERAL with unsupported node kind.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
        }
    }

    /// LATERAL select_with_parens opt_alias_clause を処理する
    ///
    /// 呼出し時、cursorはselect_with_parensを指している
    /// 呼出し後、cursorはtable_refを指している
    fn visit_lateral_subquery(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lateral_keyword: String,
        lateral_loc: Location,
    ) -> Result<TableRef, UroboroSQLFmtError> {
        ensure_kind!(cursor, SyntaxKind::select_with_parens, src);

        let sub_query_expr = self.visit_select_with_parens(cursor, src)?;

        // Expr::Sub から SubExpr を取り出す
        let sub_expr = match sub_query_expr {
            Expr::Sub(sub) => *sub,
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_lateral_subquery(): expected Sub expr from visit_select_with_parens\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        // LateralSubquery を作成
        let mut loc = lateral_loc;
        loc.append(sub_expr.loc());
        let lateral_subquery = LateralSubquery::new(&lateral_keyword, sub_expr, loc);

        let mut table_ref = Expr::LateralSubquery(Box::new(lateral_subquery)).to_aligned();

        cursor.goto_next_sibling();

        // cursor -> comment?
        // エイリアスの直前にコメントが来る場合
        if cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());

            if !comment.is_block_comment() && comment.loc().is_same_line(&table_ref.loc()) {
                table_ref.set_lhs_trailing_comment(comment)?;
            } else {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_lateral_subquery(): unexpected comment after LATERAL subquery\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            cursor.goto_next_sibling();
        }

        // cursor -> opt_alias_clause?
        if cursor.node().kind() == SyntaxKind::opt_alias_clause {
            cursor.goto_first_child();

            let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

            if let Some(as_keyword) = as_keyword {
                if CONFIG.read().unwrap().remove_table_as_keyword {
                    table_ref.add_rhs(None, col_id);
                } else {
                    table_ref.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                }
            } else {
                table_ref.add_rhs(None, col_id);
            }

            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::opt_alias_clause, src);
        }

        Ok(TableRef::SimpleTable(table_ref))
    }
}
