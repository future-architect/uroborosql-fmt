use tree_sitter::TreeCursor;

use crate::{
    config::CONFIG,
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// JOIN句
    pub(crate) fn visit_join_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // 返り値用
        let mut clauses: Vec<Clause> = vec![];

        let mut join_clause = if cursor.node().kind() == "join_type" {
            let mut clause = self.visit_join_type(cursor, src)?;
            cursor.goto_next_sibling();

            ensure_kind(cursor, "JOIN", src)?;
            clause.extend_kw(cursor.node(), src);

            clause
        } else {
            create_clause(cursor, src, "JOIN")?
        };
        cursor.goto_next_sibling();

        // キーワード直後のコメントを処理
        self.consume_comment_in_clause(cursor, src, &mut join_clause)?;

        // テーブル名だが補完は行わない
        let table = self.visit_aliasable_expr(cursor, src, None)?;
        let body = Body::from(Expr::Aligned(Box::new(table)));
        join_clause.set_body(body);

        if cursor.goto_next_sibling() {
            self.consume_comment_in_clause(cursor, src, &mut join_clause)?;
        }

        clauses.push(join_clause);

        // join_condition
        // コメント処理を行ったため、join_condition (ON ..., USING( ... ))がある場合、カーソルはjoin_conditionを指している。
        if cursor.node().kind() == "ON" || cursor.node().kind() == "USING" {
            clauses.push(self.visit_join_condition(cursor, src)?);
        }

        cursor.goto_parent();
        ensure_kind(cursor, "join_clause", src)?;

        Ok(clauses)
    }

    /// join_condition のフォーマットを行い、Clause で返す。
    /// join_condition は `ON ...` または `USING( ... )` である。
    fn visit_join_condition(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        match cursor.node().kind() {
            "ON" => {
                let mut on_clause = create_clause(cursor, src, "ON")?;
                cursor.goto_next_sibling();

                self.consume_comment_in_clause(cursor, src, &mut on_clause)?;

                let expr = self.visit_expr(cursor, src)?;
                let body = Body::from(expr);
                on_clause.set_body(body);

                Ok(on_clause)
            }
            "USING" => Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_join_clause(): JOIN USING(...) is unimplemented\n{}",
                error_annotation_from_cursor(cursor, src)
            ))),
            _ => Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_join_condition(): unimplemented node\n{}",
                error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    /// join_type の Clause を返す。
    /// join_type は次のように定義されている。
    ///
    /// ```text
    /// join_type :=
    ///     CROSS
    ///     | [NATURAL] [INNER | [LEFT | RIGHT | FULL] OUTER]
    /// ```
    ///
    /// 例えば、JOIN句 が ".. NATURAL LEFT OUTER JOIN ..." であった場合、join_type は "NATURAL LEFT OUTER"
    /// であり、これをキーワードとする Clause を返す。
    fn visit_join_type(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        if !matches!(
            cursor.node().kind(),
            "CROSS" | "NATURAL" | "INNER" | "OUTER" | "LEFT" | "RIGHT" | "FULL"
        ) {
            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_join_type(): expected node is NATURAL, INNER, OUTER, LEFT , RIGHT or FULL, but actual {}\n{}", cursor.node().kind(), error_annotation_from_cursor(cursor, src)
                )));
        }

        let mut clause = create_clause(cursor, src, cursor.node().kind())?;

        while cursor.goto_next_sibling() {
            if !matches!(
                cursor.node().kind(),
                "INNER" | "OUTER" | "LEFT" | "RIGHT" | "FULL"
            ) {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_join_type(): expected node is INNER, OUTER, LEFT, RIGHT or FULL, but actual {}\n{}", cursor.node().kind(), error_annotation_from_cursor(cursor, src)
                        )));
            }
            clause.extend_kw(cursor.node(), src);
        }

        // 省略可能であるOUTERを明示的に記載する
        //  LEFT JOIN   ->  LEFT OUTER JOIN
        //  RIGHT JOIN  ->  RIGHT OUTER JOIN
        //  FULL JOIN   ->  FULL OUTER JOIN
        if CONFIG.read().unwrap().complement_outer_keyword
            && (clause.keyword().eq_ignore_ascii_case("LEFT")
                || clause.keyword().eq_ignore_ascii_case("RIGHT")
                || clause.keyword().eq_ignore_ascii_case("FULL"))
        {
            // keyword_case = "preserve" のとき、コーディング規約に従い大文字になる。
            // keyword_case = "lower" のとき、extend_kw_with_string() で小文字に変換される
            // ため、ここでは大文字で与えてよい。
            clause.extend_kw_with_string("OUTER");
        }

        cursor.goto_parent();
        ensure_kind(cursor, "join_type", src)?;

        Ok(clause)
    }
}
