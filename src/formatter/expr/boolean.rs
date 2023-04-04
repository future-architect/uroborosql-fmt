//! 真偽値を表す式をフォーマットするメソッドを定義

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    formatter::{ensure_kind, Formatter, COMMENT},
    util::convert_keyword_case,
};

impl Formatter {
    /// bool式をフォーマットする
    /// 呼び出し後、cursorはboolean_expressionを指している
    pub(crate) fn format_bool_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        let mut boolean_expr = BooleanExpr::new("-");

        cursor.goto_first_child();

        if cursor.node().kind() == "NOT" {
            let mut loc = Location::new(cursor.node().range());
            cursor.goto_next_sibling();
            // cursor -> _expr

            // ここにバインドパラメータ以外のコメントは来ないことを想定している。
            let expr = self.format_expr(cursor, src)?;

            // (NOT expr)のソースコード上の位置を計算
            loc.append(expr.loc());

            let not_expr = UnaryExpr::new(&convert_keyword_case("NOT"), expr, loc);

            cursor.goto_parent();
            ensure_kind(cursor, "boolean_expression")?;

            // Unaryとして返す
            return Ok(Expr::Unary(Box::new(not_expr)));
        } else {
            // and or
            let left = self.format_expr(cursor, src)?;

            boolean_expr.add_expr(left);

            cursor.goto_next_sibling();
            // cursor -> COMMENT | op

            while cursor.node().kind() == COMMENT {
                boolean_expr.add_comment_to_child(Comment::new(cursor.node(), src))?;
                cursor.goto_next_sibling();
            }

            let sep = cursor.node().kind();
            boolean_expr.set_default_separator(convert_keyword_case(sep));

            cursor.goto_next_sibling();
            // cursor -> _expression

            let mut comments = vec![];
            while cursor.node().kind() == COMMENT {
                comments.push(Comment::new(cursor.node(), src));
                cursor.goto_next_sibling();
            }

            let right = self.format_expr(cursor, src)?;

            // 左辺と同様の処理を行う
            boolean_expr.add_expr_with_preceding_comments(right, comments);
        }
        // cursorをboolean_expressionに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "boolean_expression")?;

        Ok(Expr::Boolean(Box::new(boolean_expr)))
    }

    /// BETWEEN述語をフォーマットする
    /// 呼び出し後、cursorはbetween_and_expressionを指す
    pub(crate) fn format_between_and_expression(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // between_and_expressionに子供がいないことはない
        cursor.goto_first_child();
        // cursor -> expression

        let expr = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> (NOT)? BETWEEN

        let mut operator = String::new();

        if cursor.node().kind() == "NOT" {
            operator += &convert_keyword_case("NOT");
            operator += " "; // betweenの前に空白を入れる
            cursor.goto_next_sibling();
        }

        ensure_kind(cursor, "BETWEEN")?;
        operator += &convert_keyword_case("BETWEEN");
        cursor.goto_next_sibling();
        // cursor -> _expression

        let from_expr = self.format_expr(cursor, src)?;
        cursor.goto_next_sibling();

        // AND の直前に現れる行末コメントを処理する
        // 行末コメント以外のコメントは想定しない
        // TODO: 左辺に行末コメントが現れた場合のコメント縦ぞろえ
        let start_trailing_comment = if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        ensure_kind(cursor, "AND")?;
        cursor.goto_next_sibling();
        // cursor -> _expression

        let to_expr = self.format_expr(cursor, src)?;

        // (from AND to)をAlignedExprにまとめる
        let mut rhs = AlignedExpr::new(from_expr, false);
        rhs.add_rhs(convert_keyword_case("AND"), to_expr);

        if let Some(comment) = start_trailing_comment {
            rhs.set_lhs_trailing_comment(comment)?;
        }

        // (expr BETWEEN rhs)をAlignedExprにまとめる
        let mut aligned = AlignedExpr::new(expr, false);
        aligned.add_rhs(operator, Expr::Aligned(Box::new(rhs)));

        cursor.goto_parent();
        ensure_kind(cursor, "between_and_expression")?;

        Ok(aligned)
    }
}
