use tree_sitter::TreeCursor;

use crate::{
    config::CONFIG,
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{ensure_kind, Visitor, COMMENT},
};

impl Visitor {
    /// かっこで囲まれた式をフォーマットする
    /// 呼び出し後、cursorはparenthesized_expressionを指す
    pub(crate) fn visit_paren_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenExpr, UroboroSQLFmtError> {
        // parenthesized_expression: $ => PREC.unary "(" expression ")"
        // TODO: cursorを引数で渡すよう変更したことにより、tree-sitter-sqlの規則を
        //       _parenthesized_expressionに戻してもよくなったため、修正する

        let loc = Location::new(cursor.node().range());

        // 括弧の前の演算子には未対応

        cursor.goto_first_child();
        // cursor -> "("

        cursor.goto_next_sibling();
        // cursor -> comments | expr

        let mut comment_buf = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> expr

        let expr = self.visit_expr(cursor, src)?;

        let mut paren_expr = match expr {
            Expr::ParenExpr(mut paren_expr) if CONFIG.read().unwrap().remove_redundant_nest => {
                // remove_redundant_nestオプションが有効のとき、ParenExprをネストさせない
                paren_expr.set_loc(loc);
                *paren_expr
            }
            _ => ParenExpr::new(expr, loc),
        };

        // 開きかっこと式の間にあるコメントを追加
        for comment in comment_buf {
            paren_expr.add_start_comment(comment);
        }

        // かっこの中の式の最初がバインドパラメータを含む場合でも、comment_bufに読み込まれてしまう
        // そのため、現状ではこの位置のバインドパラメータを考慮していない
        cursor.goto_next_sibling();
        // cursor -> comments | ")"

        // 閉じかっこの前にあるコメントを追加
        while cursor.node().kind() == COMMENT {
            paren_expr.add_comment_to_child(Comment::new(cursor.node(), src))?;
            cursor.goto_next_sibling();
        }

        // tree-sitter-sqlを修正したら削除する
        cursor.goto_parent();
        ensure_kind(cursor, "parenthesized_expression", src)?;

        Ok(paren_expr)
    }
}
