use tree_sitter::TreeCursor;

use crate::cst::{
    AlignedExpr, Body, Clause, Comment, Expr, Location, PrimaryExpr, SeparatedLines,
    UroboroSQLFmtError,
};

use super::{create_clause, ensure_kind, Formatter, COMMENT};

impl Formatter {
    /// SELECT句
    /// 呼び出し後、cursorはselect_clauseを指している
    pub(crate) fn format_select_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // SELECT句の定義
        //    select_clause =
        //        "SELECT"
        //        select_clause_body

        // select_clauseは必ずSELECTを子供に持っているはずである
        cursor.goto_first_child();

        // cursor -> SELECT
        let mut clause = create_clause(cursor, src, "SELECT", self.state.depth)?;
        // SQL_IDとコメントを消費
        self.consume_sql_id(cursor, src, &mut clause);
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> select_caluse_body

        let body = self.format_select_clause_body(cursor, src)?;
        clause.set_body(body);

        // cursorをselect_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause")?;

        Ok(clause)
    }

    /// SELECT句の本体をSeparatedLinesで返す
    /// 呼び出し後、cursorはselect_clause_bodyを指している
    pub(crate) fn format_select_clause_body(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        // select_clause_bodyは必ず_aliasable_expressionを子供に持つ
        cursor.goto_first_child();

        // cursor -> _aliasable_expression
        // commaSep1(_aliasable_expression)
        let body = self.format_comma_sep_alias(cursor, src, false)?;

        // cursorをselect_clause_bodyに
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause_body")?;

        Ok(body)
    }

    /// FROM句をClause構造体で返す
    pub(crate) fn format_from_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clauseは必ずFROMを子供に持つ
        cursor.goto_first_child();

        // cursor -> FROM
        let mut clause = create_clause(cursor, src, "FROM", self.state.depth)?;
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> aliasable_expression
        // commaSep1(_aliasable_expression)
        let body = self.format_comma_sep_alias(cursor, src, true)?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "from_clause")?;

        Ok(clause)
    }

    pub(crate) fn format_where_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // where_clauseは必ずWHEREを子供に持つ
        cursor.goto_first_child();

        // cursor -> WHERE
        let mut clause = create_clause(cursor, src, "WHERE", self.state.depth)?;
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> _expression
        let expr = self.format_expr(cursor, src)?;

        // 結果として得られた式をBodyに変換する
        let body = Body::with_expr(expr, self.state.depth);

        clause.set_body(body);

        // cursorをwhere_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "where_clause")?;

        Ok(clause)
    }

    /// GROPU BY句に対応するClauseを持つVecを返す。
    /// HAVING句がある場合は、HAVING句に対応するClauseも含む。
    pub(crate) fn format_group_by_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        let mut clauses: Vec<Clause> = vec![];

        cursor.goto_first_child();

        let mut clause = create_clause(cursor, src, "GROUP_BY", self.state.depth)?;
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        let mut sep_lines = SeparatedLines::new(self.state.depth, ",", false);
        let first = self.format_group_expression(cursor, src)?;
        sep_lines.add_expr(first.to_aligned());

        // commaSep(group_expression)
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "," => {
                    continue;
                }
                "group_expression" => {
                    let expr = self.format_group_expression(cursor, src)?;
                    sep_lines.add_expr(expr.to_aligned());
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    sep_lines.add_comment_to_child(comment)?;
                }
                _ => {
                    break;
                }
            }
        }

        clause.set_body(Body::SepLines(sep_lines));
        clauses.push(clause);

        if cursor.node().kind() == "having_clause" {
            clauses.push(self.format_having_clause(cursor, src)?);
        }

        cursor.goto_parent();
        ensure_kind(cursor, "group_by_clause")?;

        Ok(clauses)
    }

    pub(crate) fn format_group_expression(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let ret_value = match cursor.node().kind() {
            "grouping_sets_clause" | "rollup_clause" | "cube_clause" => {
                Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_group_expression(): unimplemented node\nnode kind: {}\n{:?}",
                    cursor.node().kind(),
                    cursor.node().range()
                )))
            }
            _ => self.format_expr(cursor, src),
        };

        cursor.goto_parent();
        ensure_kind(cursor, "group_expression")?;

        ret_value
    }

    pub(crate) fn format_order_by_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // "ORDER_BY"
        let mut clause = create_clause(cursor, src, "ORDER_BY", self.state.depth)?;
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // order_expression は、左辺をカラム名、右辺をオプションとしており、演算子は常に空になる
        // そのため、is_omit_op (第3引数)に true をセットする
        let mut sep_lines = SeparatedLines::new(self.state.depth, ",", true);
        let first = self.format_order_expression(cursor, src)?;
        sep_lines.add_expr(first);

        // commaSep(order_expression)
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "order_expression" => {
                    sep_lines.add_expr(self.format_order_expression(cursor, src)?)
                }
                "," => continue,
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    sep_lines.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                        "format_order_by_clause(): unexpected node\nnode kind: {}\n{:?}",
                        cursor.node().kind(),
                        cursor.node().range()
                    )))
                }
            }
        }

        let body = Body::SepLines(sep_lines);
        clause.set_body(body);

        cursor.goto_parent();
        ensure_kind(cursor, "order_by_clause")?;

        Ok(clause)
    }

    /// ORDER BY句の本体に現れる式を AlignedExpr で返す
    /// AlignedExpr の左辺にカラム名(式)、右辺にオプション (ASC, DESC, NULLS FIRST...)を持ち、演算子は常に空にする
    pub(crate) fn format_order_expression(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();
        let expr = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();

        let order_expr = self.format_order_option(cursor, src, expr)?;

        cursor.goto_parent();
        ensure_kind(cursor, "order_expression")?;

        Ok(order_expr)
    }

    /// order_expression のオプション部分を担当する
    /// 引数に受け取った expr を左辺とする AlignedExpr を返す
    pub(crate) fn format_order_option(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        expr: Expr,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        let mut order_expr = AlignedExpr::new(expr, false);

        // オプション
        let mut order = vec![];
        // オプションの Location
        let mut order_loc = vec![];

        if matches!(cursor.node().kind(), "ASC" | "DESC") {
            let asc_or_desc = cursor.node().utf8_text(src.as_bytes()).unwrap();
            order.push(asc_or_desc);
            order_loc.push(Location::new(cursor.node().range()));

            cursor.goto_next_sibling();
        }

        if matches!(cursor.node().kind(), "NULLS") {
            let nulls = cursor.node().utf8_text(src.as_bytes()).unwrap();
            order.push(nulls);
            order_loc.push(Location::new(cursor.node().range()));
            cursor.goto_next_sibling();

            let first_or_last = cursor.node().utf8_text(src.as_bytes()).unwrap();
            order.push(first_or_last);
            order_loc.push(Location::new(cursor.node().range()));
            cursor.goto_next_sibling();
        };

        if !order.is_empty() {
            // Location を計算
            let mut loc = order_loc[0].clone();
            order_loc.into_iter().for_each(|l| loc.append(l));

            let order = PrimaryExpr::new(order.join(" "), loc);
            order_expr.add_rhs("", Expr::Primary(Box::new(order)));
        }

        Ok(order_expr)
    }

    pub(crate) fn format_having_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let mut clause = create_clause(cursor, src, "HAVING", self.state.depth)?;
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        let expr = self.format_expr(cursor, src)?;

        clause.set_body(Body::with_expr(expr, self.state.depth));

        cursor.goto_parent();
        ensure_kind(cursor, "having_clause")?;

        Ok(clause)
    }

    /// SET句をClause構造体で返す
    /// UPDATE文、INSERT文で使用する
    pub(crate) fn format_set_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        ensure_kind(cursor, "SET")?;
        let mut set_clause = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        ensure_kind(cursor, "set_clause_body")?;
        cursor.goto_first_child();

        let mut sep_lines = SeparatedLines::new(self.state.depth, ",", false);

        let mut format_set_clause_item = |cursor: &mut TreeCursor| {
            if cursor.node().kind() == "assigment_expression" {
                // tree-sitter-sqlのタイポでnが抜けている点に注意
                let aligned = self.format_assign_expr(cursor, src)?;
                Ok(aligned)
            } else if cursor.node().kind() == "(" {
                let lhs = Expr::ColumnList(Box::new(self.format_column_list(cursor, src)?));
                cursor.goto_next_sibling();
                ensure_kind(cursor, "=")?;

                cursor.goto_next_sibling();

                let rhs = if cursor.node().kind() == "select_subexpression" {
                    self.nest();
                    let expr = Expr::SelectSub(Box::new(self.format_select_subexpr(cursor, src)?));
                    self.unnest();
                    expr
                } else {
                    Expr::ColumnList(Box::new(self.format_column_list(cursor, src)?))
                };

                let mut aligned = AlignedExpr::new(lhs, false);
                aligned.add_rhs("=", rhs);

                Ok(aligned)
            } else {
                Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                    r#"format_set_clause(): expected node is assigment_expression, "(" or select_subexpression, but actual {}\n{:#?}"#,
                    cursor.node().kind(),
                    cursor.node().range()
                )))
            }
        };

        // commaSep1(set_clause_item)
        let aligned = format_set_clause_item(cursor)?;
        sep_lines.add_expr(aligned);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    sep_lines.add_comment_to_child(comment)?;
                }
                "," => continue,
                _ => {
                    let aligned = format_set_clause_item(cursor)?;
                    sep_lines.add_expr(aligned);
                }
            }
        }
        cursor.goto_parent();
        ensure_kind(cursor, "set_clause_body")?;

        // set_clauseにBodyをセット
        set_clause.set_body(Body::SepLines(sep_lines));

        cursor.goto_parent();
        ensure_kind(cursor, "set_clause")?;

        Ok(set_clause)
    }

    pub(crate) fn format_assign_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();
        let identifier = self.format_expr(cursor, src)?;
        cursor.goto_next_sibling();
        ensure_kind(cursor, "=")?;
        cursor.goto_next_sibling();
        let expr = self.format_expr(cursor, src)?;

        let mut aligned = AlignedExpr::new(identifier, false);
        aligned.add_rhs("=", expr);
        cursor.goto_parent();
        ensure_kind(cursor, "assigment_expression")?;

        Ok(aligned)
    }

    /// RETURNING句をClauseで返す
    pub(crate) fn format_returning_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        ensure_kind(cursor, "RETURNING")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        let body = self.format_comma_sep_alias(cursor, src, false)?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "returning_clause")?;

        Ok(clause)
    }
}
