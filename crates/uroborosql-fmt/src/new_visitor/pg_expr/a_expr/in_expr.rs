use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, ColumnList, Comment, Expr},
    error::UroboroSQLFmtError,
    new_visitor::pg_ensure_kind,
    util::convert_keyword_case,
};

use super::super::{pg_error_annotation_from_cursor, Visitor};

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
        pg_ensure_kind!(cursor, SyntaxKind::IN_P, src);

        // op_text: NOT IN or IN
        let op_text = if let Some(not_keyword) = not_keyword {
            let mut op_text = String::from(not_keyword);
            op_text.push(' ');

            op_text.push_str(cursor.node().text());
            op_text
        } else {
            cursor.node().text().to_string()
        };

        cursor.goto_next_sibling();

        // cursor -> comment?
        let bind_param = if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        // cursor -> in_expr
        let mut rhs = self.visit_pg_in_expr(cursor, src)?;

        if let Some(comment) = bind_param {
            if comment.is_block_comment() && comment.loc().is_next_to(&rhs.loc()) {
                rhs.set_head_comment(comment);
            } else {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_in_expr_nodes(): Unexpected comment\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(convert_keyword_case(&op_text)), rhs);

        Ok(aligned)
    }

    /// in_expr を Expr に変換する
    /// 呼出し後、cursorは in_expr を指している
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

        let expr = match cursor.node().kind() {
            SyntaxKind::select_with_parens => self.visit_select_with_parens(cursor, src)?,
            SyntaxKind::LParen => {
                // Expr::ColumnList を返す
                // '(' expr_list ')' を ColumnList に変換する
                let column_list =
                    ColumnList::from(self.handle_parenthesized_expr_list(cursor, src)?);

                Expr::ColumnList(Box::new(column_list))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_in_expr(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        // cursor -> in_expr
        pg_ensure_kind!(cursor, SyntaxKind::in_expr, src);

        Ok(expr)
    }
}
