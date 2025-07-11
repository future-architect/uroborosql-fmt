use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{ExprList, FunctionCall, FunctionCallArgs, FunctionCallKind, Location},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// TRIM関数をフォーマットする
    pub(crate) fn handle_trim_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // TRIM '(' trim_list ')'
        // TRIM '(' BOTH trim_list ')'
        // TRIM '(' LEADING trim_list ')'
        // TRIM '(' TRAILING trim_list ')'

        // cursor -> TRIM
        pg_ensure_kind!(cursor, SyntaxKind::TRIM, src);
        let keyword_text = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut arg_loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();

        // cursor -> BOTH | LEADING | TRAILING
        if matches!(
            cursor.node().kind(),
            SyntaxKind::BOTH | SyntaxKind::LEADING | SyntaxKind::TRAILING
        ) {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "handle_trim_function(): BOTH/LEADING/TRAILING is not implemented yet\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> trim_list
        pg_ensure_kind!(cursor, SyntaxKind::trim_list, src);
        let args = self.visit_trim_list(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        arg_loc.append(Location::from(cursor.node().range()));

        assert!(!cursor.goto_next_sibling());

        let function_args = FunctionCallArgs::try_from_expr_list(&args, arg_loc)?;
        let function = FunctionCall::new(
            keyword_text,
            function_args,
            FunctionCallKind::BuiltIn,
            cursor
                .node()
                .parent()
                .expect("handle_trim_function: cursor.node().parent() is None")
                .range()
                .into(),
        );

        Ok(function)
    }

    fn visit_trim_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ExprList, UroboroSQLFmtError> {
        // trim_list:
        // - a_expr FROM expr_list
        // - FROM expr_list
        // - expr_list

        cursor.goto_first_child();

        let list = match cursor.node().kind() {
            SyntaxKind::expr_list => {
                // expr_list のみ
                self.visit_expr_list(cursor, src)?
            }
            SyntaxKind::a_expr => {
                // a_expr FROM expr_list
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_trim_list(): a_expr FROM expr_list is not implemented yet\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::FROM => {
                // FROM expr_list
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_trim_list(): FROM expr_list pattern is not implemented yet\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_trim_list(): unexpected node kind {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::trim_list, src);

        Ok(list)
    }
}
