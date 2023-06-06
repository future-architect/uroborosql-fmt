//! 副問い合わせに関する式のフォーマットを定義

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    formatter::{ensure_kind, Formatter, COMMENT},
    util::convert_keyword_case,
};

impl Formatter {
    /// かっこで囲まれたSELECTサブクエリをフォーマットする
    /// 呼び出し後、cursorはselect_subexpressionを指している
    pub(crate) fn format_select_subexpr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SubExpr, UroboroSQLFmtError> {
        // select_subexpression -> "(" select_statement ")"

        let loc = Location::new(cursor.node().range());

        // cursor -> select_subexpression

        cursor.goto_first_child();
        // cursor -> (

        cursor.goto_next_sibling();
        // cursor -> comments | select_statement

        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> select_statement
        let mut select_stmt = self.format_select_stmt(cursor, src)?;

        // select_statementの前にコメントがあった場合、コメントを追加
        comment_buf
            .into_iter()
            .for_each(|c| select_stmt.add_comment(c));

        cursor.goto_next_sibling();
        // cursor -> comments | )

        while cursor.node().kind() == COMMENT {
            // 閉じかっこの直前にコメントが来る場合
            let comment = Comment::new(cursor.node(), src);
            select_stmt.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> )
        cursor.goto_parent();
        ensure_kind(cursor, "select_subexpression")?;

        Ok(SubExpr::new(select_stmt, loc))
    }

    /// EXISTSサブクエリをフォーマットする
    pub(crate) fn format_exists_subquery(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ExistsSubquery, UroboroSQLFmtError> {
        // exists_subquery_expression => "EXISTS" select_subexpression

        let exists_loc = Location::new(cursor.node().range());

        cursor.goto_first_child();
        // cursor -> "EXISTS"

        ensure_kind(cursor, "EXISTS")?;
        let exists_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

        cursor.goto_next_sibling();
        // cursor -> "select_subexpression"

        let select_subexpr = self.format_select_subexpr(cursor, src)?;

        let exists_subquery = ExistsSubquery::new(exists_keyword, select_subexpr, exists_loc);

        cursor.goto_parent();
        ensure_kind(cursor, "exists_subquery_expression")?;

        Ok(exists_subquery)
    }

    /// INサブクエリをフォーマットする
    pub(crate) fn format_in_subquery(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // in_subquery_expression => _expression "NOT"? "IN" select_subexpression

        // AlignedExprに格納
        // lhs: expression
        // op:  "NOT"? "IN"
        // rhs: select_subexpression

        cursor.goto_first_child();
        // cursor -> _expression

        let lhs = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> "NOT"?

        // NOT IN または、IN
        let mut op = String::new();
        if cursor.node().kind() == "NOT" {
            op.push_str(&convert_keyword_case(
                cursor.node().utf8_text(src.as_bytes()).unwrap(),
            ));
            op.push(' ');
            cursor.goto_next_sibling();
            // cursor -> "IN"
        }

        ensure_kind(cursor, "IN")?;
        op.push_str(&convert_keyword_case(
            cursor.node().utf8_text(src.as_bytes()).unwrap(),
        ));
        cursor.goto_next_sibling();
        // cursor -> select_subexpression

        ensure_kind(cursor, "select_subexpression")?;
        let rhs = Expr::Sub(Box::new(self.format_select_subexpr(cursor, src)?));

        let mut in_sub = AlignedExpr::new(lhs, false);
        in_sub.add_rhs(op, rhs);

        cursor.goto_parent();
        ensure_kind(cursor, "in_subquery_expression")?;

        Ok(in_sub)
    }

    /// ALLサブクエリ, SOMEサブクエリ, ANYサブクエリをフォーマットする
    pub(crate) fn format_all_some_any_subquery(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // all_some_any_subquery_expression =>
        //     expression
        //     比較演算子
        //     "ALL" | "SOME" | "ANY"
        //     select_subexpression

        // AlignedExprに格納
        // lhs: expression
        // op:  比較演算子 + \t + "ALL" | "SOME" | "ANY"
        // rhs: select_subexpression

        cursor.goto_first_child();
        // cursor -> expression

        let lhs = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> 比較演算子

        let op = cursor.node().utf8_text(src.as_ref()).unwrap();

        cursor.goto_next_sibling();
        // cursor -> "ALL" | "SOME" | "ANY"

        let all_some_any_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

        cursor.goto_next_sibling();
        // cursor -> "select_subexpression"

        let select_subexpr = self.format_select_subexpr(cursor, src)?;

        let mut all_some_any_sub = AlignedExpr::new(lhs, false);

        all_some_any_sub.add_rhs(
            format!("{op}\t{all_some_any_keyword}"),
            Expr::Sub(Box::new(select_subexpr)),
        );

        cursor.goto_parent();
        ensure_kind(cursor, "all_some_any_subquery_expression")?;

        Ok(all_some_any_sub)
    }
}
