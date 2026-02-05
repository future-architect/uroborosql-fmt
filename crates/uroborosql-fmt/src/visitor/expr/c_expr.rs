mod aexpr_const;
mod array_expr;
mod case_expr;
mod columnref;
mod exists_subquery;
mod func_expr;
mod implicit_row;
mod select_with_parens;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Expr, ParenExpr},
    error::UroboroSQLFmtError,
    visitor::ensure_kind,
    CONFIG,
};

use super::{error_annotation_from_cursor, Visitor};

/*
 * c_expr の構造
 *
 * - columnref
 * - AexprConst
 * - PARAM opt_indirection
 * - '(' a_expr ')' opt_indirection
 * - case_expr
 * - func_expr
 * - select_with_parens
 * - select_with_parens indirection
 * - EXISTS select_with_parens
 * - ARRAY select_with_parens
 * - ARRAY array_expr
 * - explicit_row
 * - implicit_row
 * - GROUPING '(' expr_list ')'
 */

impl Visitor {
    pub fn visit_c_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::columnref => self.visit_columnref(cursor, src)?,
            SyntaxKind::AexprConst => self.visit_aexpr_const(cursor, src)?,
            SyntaxKind::PARAM => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): PARAM is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::LParen => {
                // '(' a_expr ')' opt_indirection

                // cursor -> '('
                ensure_kind!(cursor, SyntaxKind::LParen, src);

                cursor.goto_next_sibling();
                // cursor -> comments?

                let mut start_comment_buf = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    start_comment_buf.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> expr
                ensure_kind!(cursor, SyntaxKind::a_expr, src);
                let mut expr = self.visit_a_expr_or_b_expr(cursor, src)?;

                cursor.goto_next_sibling();

                let mut end_comment_buf = vec![];

                // cursor -> comment?
                // 式の行末コメントならば式に付与し、そうでなければ閉じ括弧の前のコメントとして保持する
                if cursor.node().kind() == SyntaxKind::SQL_COMMENT {
                    let comment = Comment::new(cursor.node());

                    if comment.loc().is_same_line(&expr.loc()) {
                        expr.add_comment_to_child(comment)?;
                    } else {
                        end_comment_buf.push(comment);
                    }

                    cursor.goto_next_sibling();
                }

                // cursor -> comments?
                // 閉じ括弧の前のコメントを処理する
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    end_comment_buf.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> ")"
                ensure_kind!(cursor, SyntaxKind::RParen, src);

                // 親(c_expr) の location を設定
                // 親が無いことはありえないので、parent() の返り値が None の場合は panic する
                let parent_loc = cursor
                    .node()
                    .parent()
                    .expect("visit_c_expr(): parent is None")
                    .range()
                    .into();

                let mut paren_expr = match expr {
                    Expr::ParenExpr(mut paren_expr)
                        if CONFIG.read().unwrap().remove_redundant_nest =>
                    {
                        // remove_redundant_nestオプションが有効のとき、ParenExprをネストさせない
                        paren_expr.set_loc(parent_loc);
                        *paren_expr
                    }
                    _ => ParenExpr::new(expr, parent_loc),
                };

                // 開きかっこと式の間にあるコメントを追加
                for comment in start_comment_buf {
                    paren_expr.add_start_comment(comment);
                }

                // 式から閉じかっこの間にあるコメントを追加
                for comment in end_comment_buf {
                    paren_expr.add_end_comment(comment);
                }

                Expr::ParenExpr(Box::new(paren_expr))
            }
            SyntaxKind::case_expr => {
                let cond_expr = self.visit_case_expr(cursor, src)?;
                Expr::Cond(Box::new(cond_expr))
            }
            SyntaxKind::func_expr => self.visit_func_expr(cursor, src)?,
            SyntaxKind::select_with_parens => self.visit_select_with_parens(cursor, src)?,
            SyntaxKind::EXISTS => {
                let exists_subquery = self.handle_exists_subquery_nodes(cursor, src)?;
                Expr::ExistsSubquery(Box::new(exists_subquery))
            }
            SyntaxKind::ARRAY => {
                let array_expr = self.handle_array_nodes(cursor, src)?;
                Expr::ArrayExpr(Box::new(array_expr))
            }
            SyntaxKind::explicit_row => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): explicit_row is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::implicit_row => {
                let implicit_row = self.visit_implicit_row(cursor, src)?;
                Expr::ColumnList(Box::new(implicit_row))
            }
            SyntaxKind::GROUPING => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): GROUPING is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_c_expr(): unexpected syntaxkind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::c_expr, src);

        Ok(expr)
    }
}
