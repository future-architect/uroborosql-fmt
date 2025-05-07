use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{Comment, Expr, Location, ParenExpr, SubExpr},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
    CONFIG,
};

use super::Visitor;

impl Visitor {
    pub(crate) fn visit_select_with_parens(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // select_with_parens
        //  ├ '(' select_no_parens ')'
        //  └ '(' select_with_parens ')'

        // 全体の位置情報を保持
        let loc = Location::from(cursor.node().range());

        cursor.goto_first_child();
        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();

        // cursor -> comments?
        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> select_no_parens | select_with_parens
        pg_ensure_kind!(
            cursor,
            SyntaxKind::select_no_parens | SyntaxKind::select_with_parens,
            src
        );
        let mut expr = match cursor.node().kind() {
            SyntaxKind::select_no_parens => {
                let statement = self.visit_select_no_parens(cursor, src)?;
                let mut sub_expr = SubExpr::new(statement, loc);

                // 開きかっことSELECT文の間にあるコメントを追加
                for comment in comment_buf {
                    sub_expr.add_start_comment(comment)?;
                }

                cursor.goto_next_sibling();

                Expr::Sub(Box::new(sub_expr))
            }
            SyntaxKind::select_with_parens => {
                // ネストした select_with_parens は ParenExpr で表現する
                let select_with_parens = self.visit_select_with_parens(cursor, src)?;

                // remove_redundant_nest オプションが有効のとき、 ParenExpr をネストさせない
                let mut paren_expr = match select_with_parens {
                    Expr::ParenExpr(mut paren_expr)
                        if CONFIG.read().unwrap().remove_redundant_nest =>
                    {
                        paren_expr.set_loc(loc);
                        *paren_expr
                    }
                    _ => ParenExpr::new(select_with_parens, loc),
                };

                // 開きかっこと式の間にあるコメントを追加
                for comment in comment_buf {
                    paren_expr.add_start_comment(comment);
                }

                cursor.goto_next_sibling();

                Expr::ParenExpr(Box::new(paren_expr))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_select_with_parens(): {} node appeared. This node is not considered yet.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        // 閉じ括弧の前にあるコメントを追加
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            expr.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::select_with_parens, src);

        Ok(expr)
    }
}
