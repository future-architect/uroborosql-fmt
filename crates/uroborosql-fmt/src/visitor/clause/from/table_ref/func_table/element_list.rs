use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        table_function_alias::element_list::{ParenthesizedTableFuncElementList, TableFuncElement},
        Comment, Location, PrimaryExpr, PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// 括弧で囲まれた TableFuncElementList を走査する
    /// 呼出し時、cursor は '(' を指している
    /// 呼出し後、cursor は ')' を指している
    pub(crate) fn handle_parenthesized_table_func_element_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenthesizedTableFuncElementList, UroboroSQLFmtError> {
        // '(' TableFuncElementList ')'
        // ^^^                      ^^^
        // 呼出し時                  呼出し後

        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> comment?
        let mut start_comments = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            start_comments.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> TableFuncElementList
        ensure_kind!(cursor, SyntaxKind::TableFuncElementList, src);
        let mut exprs = self.visit_table_func_element_list(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> comment?

        if cursor.node().is_comment() {
            // 行末コメントを想定する
            let comment = Comment::new(cursor.node());

            // exprs は必ず1つ以上要素を持っている
            let last = exprs.last_mut().unwrap();
            if last.loc().is_same_line(&comment.loc()) {
                last.set_trailing_comment(comment);
            } else {
                // 行末コメント以外のコメントは想定していない
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_parenthesized_table_func_element_list(): Unexpected comment\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        loc.append(Location::from(cursor.node().range()));

        Ok(ParenthesizedTableFuncElementList::new(
            exprs,
            loc,
            start_comments,
        ))
    }

    fn visit_table_func_element_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<TableFuncElement>, UroboroSQLFmtError> {
        // TableFuncElementList:
        // - TableFuncElement ( ',' TableFuncElementList )*
        //
        // this node is flatten: https://github.com/future-architect/postgresql-cst-parser/pull/29

        cursor.goto_first_child();
        // cursor -> TableFuncElement

        let mut exprs = vec![];

        ensure_kind!(cursor, SyntaxKind::TableFuncElement, src);
        let first = self.visit_table_func_element(cursor, src)?;
        exprs.push(first);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::TableFuncElement => {
                    let expr = self.visit_table_func_element(cursor, src)?;
                    exprs.push(expr);
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());

                    // exprs は必ず1つ以上要素を持っている
                    let last = exprs.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment);
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_table_func_element_list(): Unexpected comment\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_table_func_element_list(): unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::TableFuncElementList, src);

        Ok(exprs)
    }

    fn visit_table_func_element(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<TableFuncElement, UroboroSQLFmtError> {
        // TableFuncElement:
        // - ColId Typename opt_collate_clause?

        cursor.goto_first_child();

        // cursor -> ColId
        ensure_kind!(cursor, SyntaxKind::ColId, src);
        let col_id = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;

        cursor.goto_next_sibling();
        // cursor -> Typename

        ensure_kind!(cursor, SyntaxKind::Typename, src);
        let typename = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;

        cursor.goto_next_sibling();
        // cursor -> opt_collate_clause?

        if cursor.node().kind() == SyntaxKind::opt_collate_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_table_func_element(): collate clause in function alias is not implemented yet.\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();
        // cursor -> TableFuncElement
        ensure_kind!(cursor, SyntaxKind::TableFuncElement, src);
        let loc = Location::from(cursor.node().range());

        Ok(TableFuncElement::new(col_id, typename, loc))
    }
}
