use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, FunctionCall, FunctionCallArgs, FunctionCallKind, Location},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// SUBSTRING 関数をフォーマットする
    pub(crate) fn handle_substring_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // SUBSTRING '(' func_arg_list_opt ')'
        // SUBSTRING '(' substr_list ')'

        // cursor -> SUBSTRING
        ensure_kind!(cursor, SyntaxKind::SUBSTRING, src);
        let keyword_text = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);

        let mut args = FunctionCallArgs::new(vec![], Location::from(cursor.node().range()));
        cursor.goto_next_sibling();

        // cursor -> C_COMMENT?
        let comment_before_first_argument = if cursor.node().kind() == SyntaxKind::C_COMMENT {
            let comment = Comment::new(cursor.node());
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        // cursor -> substr_list | func_arg_list_opt
        match cursor.node().kind() {
            SyntaxKind::substr_list => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_substring_function(): substr_list is not implemented yet\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::func_arg_list_opt => {
                // func_arg_list_opt:
                // - func_arg_list

                cursor.goto_first_child();

                ensure_kind!(cursor, SyntaxKind::func_arg_list, src);
                self.visit_func_arg_list(cursor, src, &mut args, comment_before_first_argument)?;

                cursor.goto_parent();
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_substring_function(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        cursor.goto_next_sibling();
        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        args.append_loc(Location::from(cursor.node().range()));

        assert!(!cursor.goto_next_sibling());

        let function = FunctionCall::new(
            keyword_text,
            args,
            FunctionCallKind::BuiltIn,
            cursor
                .node()
                .parent()
                .expect("handle_substring_function: cursor.node().parent() is None")
                .range()
                .into(),
        );

        Ok(function)
    }
}
