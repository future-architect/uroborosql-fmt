mod arithmetic;
mod comparison;
mod in_expr;
mod is_expr;
mod like;
mod logical;
mod type_cast;
mod unary;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Expr},
    error::UroboroSQLFmtError,
};

use super::{pg_error_annotation_from_cursor, AExprOrBExpr, Visitor};

impl Visitor {
    /// a_expr の 子ノードを走査する
    /// 呼出し時、cursor は a_expr の最初の子ノードを指している
    /// 呼出し後、cursor は a_expr の最後の子ノードを指している
    pub fn handle_a_expr_inner(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // cursor -> c_expr | DEFAULT | Plus | Minus | NOT | qual_Op | a_expr | UNIQUE
        match cursor.node().kind() {
            SyntaxKind::c_expr => self.visit_c_expr(cursor, src),
            SyntaxKind::DEFAULT => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_a_expr_or_b_expr(): DEFAULT is not implemented".to_string(),
                ))
            }
            // Unary Expression
            SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::NOT | SyntaxKind::qual_Op => {
                let unary = self.handle_unary_expr_nodes(cursor, src)?;
                Ok(Expr::Unary(Box::new(unary)))
            }
            SyntaxKind::a_expr => {
                // cursor -> a_expr
                let mut lhs = self.visit_a_expr_or_b_expr(cursor, src, AExprOrBExpr::AExpr)?;

                cursor.goto_next_sibling();
                // cursor -> comment?
                if cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    lhs.add_comment_to_child(comment)?;

                    cursor.goto_next_sibling();
                }

                // cursor -> 算術演算子 | 比較演算子 | 論理演算子 | TYPECAST | COLLATE | AT | LIKE | ILIKE | SIMILAR | IS | ISNULL | NOTNULL | IN | サブクエリ
                let expr = self.handle_nodes_after_a_expr(cursor, src, lhs)?;

                Ok(expr)
            }
            SyntaxKind::row => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::UNIQUE => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_a_expr_or_b_expr(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
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
        // cursor -> 算術演算子 | 比較演算子 | 論理演算子 | TYPECAST | COLLATE | AT | LIKE | ILIKE | SIMILAR | IS | ISNULL | NOTNULL | IN | サブクエリ
        match cursor.node().kind() {
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
                let sep_lines = self.handle_logical_expr_nodes(cursor, src, lhs)?;
                Ok(Expr::Boolean(Box::new(sep_lines)))
            }
            // 型変換
            SyntaxKind::TYPECAST => {
                let expr = self.handle_typecast_nodes(cursor, src, lhs)?;
                Ok(expr)
            }
            // 属性関連
            SyntaxKind::COLLATE | SyntaxKind::AT => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
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
            SyntaxKind::SIMILAR => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            // IS
            SyntaxKind::IS => {
                let aligned = self.handle_is_expr_nodes(cursor, src, lhs)?;
                Ok(Expr::Aligned(Box::new(aligned)))
            }
            SyntaxKind::ISNULL | SyntaxKind::NOTNULL => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
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
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            // サブクエリ
            SyntaxKind::subquery_Op => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
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
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
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
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr_or_b_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    _ => {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_a_expr_or_b_expr(): Unexpected syntax. node: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_a_expr_or_b_expr(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }
    }
}
