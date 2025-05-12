mod all_some_any_subquery;
mod arithmetic;
mod between;
mod comparison;
mod in_expr;
mod is_expr;
mod like;
mod logical;
mod type_cast;
mod unary;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Expr, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    util::create_error_annotation,
};

use super::{pg_error_annotation_from_cursor, Visitor};

impl Visitor {
    /// a_expr または b_expr の 子ノードを走査する
    /// 呼出し時、cursor は a_expr または b_expr の最初の子ノードを指している
    /// 呼出し後、cursor は a_expr または b_expr の最後の子ノードを指している
    pub fn handle_a_expr_or_b_expr_inner(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // cursor -> c_expr | DEFAULT | Plus | Minus | NOT | qual_Op | a_expr | UNIQUE
        match cursor.node().kind() {
            SyntaxKind::c_expr => self.visit_c_expr(cursor, src),
            SyntaxKind::DEFAULT => {
                let primary = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;
                Ok(Expr::Primary(Box::new(primary)))
            }
            // Unary Expression
            SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::NOT | SyntaxKind::qual_Op => {
                let unary = self.handle_unary_expr_nodes(cursor, src)?;
                Ok(Expr::Unary(Box::new(unary)))
            }
            SyntaxKind::a_expr => {
                // cursor -> a_expr
                let lhs = self.visit_a_expr_or_b_expr(cursor, src)?;

                cursor.goto_next_sibling();

                // cursor -> コメント | 算術演算子 | 比較演算子 | 論理演算子 | TYPECAST | COLLATE | AT | LIKE | ILIKE | SIMILAR | IS | ISNULL | NOTNULL | IN | サブクエリ
                let expr = self.handle_nodes_after_a_expr(cursor, src, lhs)?;

                Ok(expr)
            }
            SyntaxKind::row => Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                cursor.node().kind(),
                pg_error_annotation_from_cursor(cursor, src)
            ))),
            SyntaxKind::UNIQUE => Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                cursor.node().kind(),
                pg_error_annotation_from_cursor(cursor, src)
            ))),
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_a_expr_or_b_expr(): Unexpected syntax. node: {}\n{}",
                cursor.node().kind(),
                pg_error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    /// a_expr の子ノードのうち、最初に a_expr が現れた後のノードを走査する
    /// 呼出時、cursor は a_expr の次のノードを指している
    /// 呼出後、cursor は a_expr の最後の子ノードを指している
    fn handle_nodes_after_a_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // この位置（演算子の直前など）に現れるコメントを追加できるかどうかは、返す式の種類が決まってからでないと確定しない
        // コメントを追加したらベクタから消去し、最後にコメントが残っていないかどうかチェックする
        let mut comments_before_op = vec![];
        while cursor.node().is_comment() {
            comments_before_op.push(Comment::pg_new(cursor.node()));
            cursor.goto_next_sibling();
        }

        // cursor -> 算術演算子 | 比較演算子 | 論理演算子 | TYPECAST | COLLATE | AT | LIKE | ILIKE | SIMILAR | IS | ISNULL | NOTNULL | IN | サブクエリ
        let result = match cursor.node().kind() {
            // 算術演算
            SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Percent
            | SyntaxKind::Caret => {
                let seq = self.handle_arithmetic_binary_expr_nodes(cursor, src, lhs)?;
                Ok(Expr::ExprSeq(Box::new(seq)))
            }
            // 比較演算
            SyntaxKind::Less
            | SyntaxKind::Greater
            | SyntaxKind::Equals
            | SyntaxKind::LESS_EQUALS
            | SyntaxKind::GREATER_EQUALS
            | SyntaxKind::NOT_EQUALS
            | SyntaxKind::qual_Op => {
                let aligned = self.handle_comparison_expr_nodes(cursor, src, lhs)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            // 論理
            SyntaxKind::AND | SyntaxKind::OR => {
                let sep_lines = self.handle_logical_expr_nodes(
                    cursor,
                    src,
                    lhs,
                    std::mem::take(&mut comments_before_op),
                )?;

                Ok(Expr::Boolean(Box::new(sep_lines)))
            }
            // 型変換
            SyntaxKind::TYPECAST => {
                let expr = self.handle_typecast_nodes(cursor, src, lhs)?;
                Ok(expr)
            }
            // 属性関連
            SyntaxKind::COLLATE | SyntaxKind::AT => {
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::LIKE | SyntaxKind::ILIKE => {
                // LIKE, ILIKE は同じ構造
                let aligned = self.handle_like_expr_nodes(cursor, src, lhs, None)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            SyntaxKind::SIMILAR => Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                cursor.node().kind(),
                pg_error_annotation_from_cursor(cursor, src)
            ))),
            // IS
            SyntaxKind::IS => {
                let aligned = self.handle_is_expr_nodes(cursor, src, lhs)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            SyntaxKind::ISNULL | SyntaxKind::NOTNULL => {
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            // IN
            SyntaxKind::IN_P => {
                // IN_P in_expr
                let aligned = self.handle_in_expr_nodes(cursor, src, lhs, None)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            // BETWEEN
            SyntaxKind::BETWEEN => {
                let aligned = self.handle_between_expr_nodes(cursor, src, lhs, None)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            // ALL, ANY, SOME 式
            SyntaxKind::subquery_Op => {
                let aligned = self.handle_all_some_any_nodes(cursor, src, lhs)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            SyntaxKind::NOT_LA => {
                // NOT キーワードのうち、 後に BETWEEN, IN, LIKE, ILIKE, SIMILAR のいずれかが続くケース
                // cursor -> NOT_LA
                let not_text = cursor.node().text();

                cursor.goto_next_sibling();
                // cursor -> BETWEEN | IN | LIKE | ILIKE | SIMILAR

                match cursor.node().kind() {
                    SyntaxKind::BETWEEN => {
                        // NOT_LA BETWEEN
                        let aligned =
                            self.handle_between_expr_nodes(cursor, src, lhs, Some(not_text))?;
                        Ok(Expr::Aligned(Box::new(aligned)))
                    }
                    SyntaxKind::IN_P => {
                        // NOT_LA IN_P
                        let aligned =
                            self.handle_in_expr_nodes(cursor, src, lhs, Some(not_text))?;
                        Ok(Expr::Aligned(Box::new(aligned)))
                    }
                    SyntaxKind::LIKE | SyntaxKind::ILIKE => {
                        // NOT_LA LIKE
                        // NOT_LA ILIKE

                        // ILIKE も LIKE と同じ構造
                        let aligned =
                            self.handle_like_expr_nodes(cursor, src, lhs, Some(not_text))?;
                        Ok(Expr::Aligned(Box::new(aligned)))
                    }
                    SyntaxKind::SIMILAR => {
                        // NOT_LA SIMILAR
                        Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_a_expr_or_b_expr(): Unexpected syntax. node: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    ))),
                }
            }
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_a_expr_or_b_expr(): Unexpected syntax. node: {}\n{}",
                cursor.node().kind(),
                pg_error_annotation_from_cursor(cursor, src)
            ))),
        };

        // comment が残っている場合はエラー
        if !comments_before_op.is_empty() {
            let first_comment = comments_before_op.first().unwrap();

            let text = first_comment.text();
            let location = first_comment.loc();

            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "handle_nodes_after_a_expr(): Unexpected comment. comment: `{}`\n{}",
                text,
                create_error_annotation(&location, "This comment is not consumed.", src)?
            )));
        }

        result
    }
}
