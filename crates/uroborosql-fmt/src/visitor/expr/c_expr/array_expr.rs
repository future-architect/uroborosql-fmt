use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{ArrayExpr, Comment, Expr, Location},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor},
};

use super::Visitor;

impl Visitor {
    /// Handles ARRAY array_expr or ARRAY select_with_parens nodes
    ///
    /// Called when cursor is at ARRAY keyword.
    /// After call, cursor is at the last child (array_expr or select_with_parens).
    pub fn handle_array_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ArrayExpr, UroboroSQLFmtError> {
        // ARRAY array_expr
        // ^     ^
        // |     └ After call
        // └ At call

        // Get the location of the entire c_expr (parent)
        let array_loc = Location::from(
            cursor
                .node()
                .parent()
                .expect("handle_array_nodes: parent not found")
                .range(),
        );

        // cursor -> ARRAY
        ensure_kind!(cursor, SyntaxKind::ARRAY, src);
        let array_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> array_expr or select_with_parens

        match cursor.node().kind() {
            SyntaxKind::array_expr => {
                let elements = self.visit_array_expr(cursor, src)?;
                Ok(ArrayExpr::new(array_keyword, elements, array_loc))
            }
            SyntaxKind::select_with_parens => {
                // ARRAY(SELECT ...) is not yet supported
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_array_nodes(): ARRAY subquery (ARRAY(SELECT ...)) is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "handle_array_nodes(): unexpected node kind after ARRAY\n{}",
                error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    /// Visits the array_expr node and returns the list of elements
    ///
    /// array_expr:
    /// - '[' expr_list? ']'
    /// - '[' array_expr_list ']'  (for nested arrays)
    ///
    /// Called when cursor is at array_expr.
    /// After call, cursor is still at array_expr.
    fn visit_array_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<crate::cst::AlignedExpr>, UroboroSQLFmtError> {
        ensure_kind!(cursor, SyntaxKind::array_expr, src);

        cursor.goto_first_child();
        // cursor -> LBracket
        ensure_kind!(cursor, SyntaxKind::LBracket, src);

        cursor.goto_next_sibling();
        // cursor -> expr_list | array_expr_list | comment | RBracket

        // Handle comments before the expression list
        let mut leading_comments: Vec<Comment> = Vec::new();
        while cursor.node().is_comment() {
            leading_comments.push(Comment::new(cursor.node()));
            cursor.goto_next_sibling();
        }

        // cursor -> expr_list | array_expr_list | RBracket
        let elements = match cursor.node().kind() {
            SyntaxKind::expr_list => {
                let mut expr_list = self.visit_expr_list(cursor, src)?;

                // leading_comments のうち最後のものは、最初の要素のバインドパラメータの可能性がある
                if let Some(comment) = leading_comments.last() {
                    if comment.is_block_comment() {
                        if let Some(first) = expr_list.first_expr_mut() {
                            if comment.loc().is_next_to(&first.loc()) {
                                first.set_head_comment(comment.clone());
                                leading_comments.pop();
                            }
                        }
                    }
                }

                if !leading_comments.is_empty() {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_array_expr(): leading comments are not supported\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                cursor.goto_next_sibling();
                // cursor -> comment? | RBracket
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    expr_list.add_comment_to_last_item(comment)?;
                    cursor.goto_next_sibling();
                }

                // cursor -> RBracket
                ensure_kind!(cursor, SyntaxKind::RBracket, src);

                let mut elements = Vec::new();
                for item in expr_list.items() {
                    if let Some(following_comment) = item.following_comments().first() {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "Comments following elements in ARRAY are not supported. Only trailing comments are supported.\ncomment: {}",
                            following_comment.text()
                        )));
                    }
                    elements.push(item.expr().clone());
                }

                elements
            }
            SyntaxKind::array_expr_list => {
                if !leading_comments.is_empty() {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_array_expr(): leading comments before nested array are not supported\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                // Nested arrays: array[array[1,2], array[3,4]]
                let elements = self.visit_array_expr_list(cursor, src)?;
                cursor.goto_next_sibling();

                // cursor -> comment? | RBracket
                if cursor.node().is_comment() {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_array_expr(): trailing comments after nested array are not supported\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                // cursor -> RBracket
                ensure_kind!(cursor, SyntaxKind::RBracket, src);

                elements
            }
            SyntaxKind::RBracket => {
                if !leading_comments.is_empty() {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_array_expr(): leading comments on empty array are not supported\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                // Empty array: array[]
                vec![]
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_array_expr(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        // cursor -> array_expr

        Ok(elements)
    }

    /// Visits array_expr_list (for nested arrays)
    fn visit_array_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<crate::cst::AlignedExpr>, UroboroSQLFmtError> {
        ensure_kind!(cursor, SyntaxKind::array_expr_list, src);

        cursor.goto_first_child();
        // cursor -> array_expr | Comma

        let mut elements = vec![];

        loop {
            match cursor.node().kind() {
                SyntaxKind::array_expr => {
                    // Get the location of this nested array_expr
                    let nested_loc = Location::from(cursor.node().range());

                    // Visit the nested array_expr to get its elements
                    let nested_elements = self.visit_array_expr(cursor, src)?;

                    // Create an ArrayExpr for the nested array (without ARRAY keyword)
                    // Since nested arrays don't have the ARRAY keyword, we just format them as [...]
                    let nested_array = ArrayExpr::new(
                        String::new(), // Empty keyword for nested arrays
                        nested_elements,
                        nested_loc,
                    );

                    // Wrap in Expr and then AlignedExpr
                    let expr = Expr::ArrayExpr(Box::new(nested_array));
                    elements.push(expr.to_aligned());
                }
                SyntaxKind::Comma => {
                    // Skip comma
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_array_expr_list(): unexpected node kind\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        cursor.goto_parent();
        // cursor -> array_expr_list

        Ok(elements)
    }
}
