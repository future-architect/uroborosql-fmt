use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, ColumnList, Expr, Location},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
};

use super::super::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor};

impl Visitor {
    /// 左辺の式とオプションのNOTキーワードを受け取り、IN述語にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は IN_P を指している
    /// 呼出後、cursor は in_expr （同階層の最後の要素）を指している
    ///
    pub fn handle_in_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
        not_keyword: Option<&str>,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // a_expr NOT? IN_P in_expr
        // ^      ^    ^    ^
        // lhs    |    │    └ 呼出後
        //        |    └ 呼出時
        //        |
        //        └ not_keyword

        // cursor -> IN_P
        pg_ensure_kind(cursor, SyntaxKind::IN_P, src)?;

        // op_text: NOT IN or IN
        let op_text = if let Some(not_keyword) = not_keyword {
            let mut op_text = String::from(not_keyword);
            op_text.push(' ');

            op_text.push_str(cursor.node().text());
            op_text
        } else {
            cursor.node().text().to_string()
        };

        // TODO: バインドパラメータ対応

        cursor.goto_next_sibling();
        // cursor -> in_expr
        let rhs = self.visit_pg_in_expr(cursor, src)?;

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(convert_keyword_case(&op_text)), rhs);

        Ok(aligned)
    }

    /// in_expr を Expr に変換する
    /// 呼出し後、cursorは in_expr を指している
    ///
    /// AlignedExpr になるための右辺を返す
    /// - select_with_parens の場合は Expr::Sub
    ///   - visitor::Visitor::visit_in_subquery に対応
    /// - '(' expr_list ')'の場合は Expr::ColumnList
    ///   - visitor::Visitor::visit_in_expr に対応
    ///
    fn visit_pg_in_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // in_expr
        //   - select_with_parens
        //   - '(' expr_list ')'

        // cursor -> in_expr
        cursor.goto_first_child();
        // cursor -> select_with_parens | '('

        match cursor.node().kind() {
            SyntaxKind::select_with_parens => {
                // Expr::Sub を返す
                // Ok(Expr::Sub(Box::new(subquery)))
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_in_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::LParen => {
                // Expr::ColumnList を返す
                // '(' expr_list ')' を ColumnList に変換する
                let column_list = self.visit_parenthesized_expr_list(cursor, src)?;
                cursor.goto_parent();
                // cursor -> in_expr

                Ok(Expr::ColumnList(Box::new(column_list)))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_in_expr(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }
    }

    /// '(' expr_list ')' を ColumnList に変換する
    /// parenthesized_expr_list というノードは存在しない
    /// 呼出し後、cursor は RParen ')' を指している
    fn visit_parenthesized_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // parenthesized_expr_list
        //   - '(' expr_list ')'
        //
        // expr_list
        //   - a_expr (',' a_expr)*
        //
        // expr_list はフラット化されている:
        // https://github.com/future-architect/postgresql-cst-parser/pull/10

        // TODO: コメント処理

        // cursor -> '('
        pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;

        cursor.goto_next_sibling();
        // cursor -> expr_list

        cursor.goto_first_child();
        // cursor -> a_expr | ','

        let mut exprs = Vec::new();

        // 最初の要素
        if cursor.node().kind() == SyntaxKind::a_expr {
            exprs.push(self.visit_a_expr(cursor, src)?.to_aligned());
        }

        // 残りの要素
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    exprs.push(self.visit_a_expr(cursor, src)?.to_aligned());
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_parenthesized_expr_list(): Unexpected syntax. node: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> expr_list

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

        let parent = cursor
            .node()
            .parent()
            .expect("visit_parenthesized_expr_list(): parent not found");
        let loc = Location::from(parent.range());

        // TODO: コメント処理
        Ok(ColumnList::new(exprs, loc, vec![]))
    }
}
