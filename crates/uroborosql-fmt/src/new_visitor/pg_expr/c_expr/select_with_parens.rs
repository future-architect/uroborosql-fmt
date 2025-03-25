use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Expr, Location, ParenExpr, SubExpr},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
    CONFIG,
};

use super::Visitor;

impl Visitor {
    /// かっこで囲まれたSELECTサブクエリをフォーマットする
    /// 呼び出し後、cursor は select_subexpression を指している
    pub fn visit_select_with_parens(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // select_with_parens
        // - '(' select_no_parens ')'
        // - '(' select_with_parens ')'
        //
        // select_no_parens というノードは実際には存在しない（cst-parser で消去される）
        // そのため、かっこの中に通常の select 文の要素が並ぶと考えればよい

        let loc = Location::from(cursor.node().range());

        // cursor -> select_with_parens

        cursor.goto_first_child();
        // cursor -> '('

        cursor.goto_next_sibling();

        // cursor -> comments?
        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> SELECT keyword | select_with_parens
        let expr = match cursor.node().kind() {
            SyntaxKind::SELECT => {
                // SelectStmt の子要素にあたるノード群が並ぶ
                // 呼出し後、cursor は ')' を指す
                let mut select_stmt = self.visit_select_stmt_inner(cursor, src)?;
                pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

                // select 文の前にコメントがあった場合、コメントを追加
                comment_buf
                    .into_iter()
                    .for_each(|c| select_stmt.add_comment(c));

                // 閉じかっこの前にあるコメントは visit_select_stmt_inner で処理済み
                let sub_expr = SubExpr::new(select_stmt, loc);

                Expr::Sub(Box::new(sub_expr))
            }
            // ネストした select_with_parens は ParenExpr で表現する
            SyntaxKind::select_with_parens => {
                let select_with_parens = self.visit_select_with_parens(cursor, src)?;
                let mut paren_expr = match select_with_parens {
                    Expr::ParenExpr(mut paren_expr)
                        if CONFIG.read().unwrap().remove_redundant_nest =>
                    {
                        // remove_redundant_nest オプションが有効のとき、 ParenExpr をネストさせない
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

                // cursor -> comments?
                // 閉じかっこの前にあるコメントを追加
                while cursor.node().is_comment() {
                    paren_expr.add_comment_to_child(Comment::pg_new(cursor.node()))?;
                    cursor.goto_next_sibling();
                }

                Expr::ParenExpr(Box::new(paren_expr))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_select_with_parens(): unexpected syntax kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::select_with_parens, src)?;

        Ok(expr)
    }
}
