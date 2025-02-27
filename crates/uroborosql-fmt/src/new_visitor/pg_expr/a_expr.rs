use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{unary::UnaryExpr, Expr, ExprSeq, Location, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
};

use super::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor};

/*
 * a_expr の構造
 *
 * 1. 基本式
 * - c_expr
 * - DEFAULT
 *
 * 2. 単項演算子
 * - '+' a_expr
 * - '-' a_expr
 * - NOT a_expr
 * - NOT_LA a_expr
 * - qual_Op a_expr
 *
 * 3. 二項算術演算子
 * - a_expr '+' a_expr
 * - a_expr '-' a_expr
 * - a_expr '*' a_expr
 * - a_expr '/' a_expr
 * - a_expr '%' a_expr
 * - a_expr '^' a_expr
 *
 * 4. 比較演算子
 * - a_expr '<' a_expr
 * - a_expr '>' a_expr
 * - a_expr '=' a_expr
 * - a_expr LESS_EQUALS a_expr
 * - a_expr GREATER_EQUALS a_expr
 * - a_expr NOT_EQUALS a_expr
 * - a_expr qual_Op a_expr
 * - a_expr IS DISTINCT FROM a_expr
 * - a_expr IS NOT DISTINCT FROM a_expr
 *
 * 5. 論理演算子
 * - a_expr AND a_expr
 * - a_expr OR a_expr
 *
 * 6. 型変換・属性関連
 * - a_expr TYPECAST Typename
 * - a_expr COLLATE any_name
 * - a_expr AT TIME ZONE a_expr
 * - a_expr AT LOCAL
 *
 * 7. パターンマッチング
 * - a_expr LIKE a_expr
 * - a_expr LIKE a_expr ESCAPE a_expr
 * - a_expr NOT_LA LIKE a_expr
 * - a_expr NOT_LA LIKE a_expr ESCAPE a_expr
 * - a_expr ILIKE a_expr
 * - a_expr ILIKE a_expr ESCAPE a_expr
 * - a_expr NOT_LA ILIKE a_expr
 * - a_expr NOT_LA ILIKE a_expr ESCAPE a_expr
 * - a_expr SIMILAR TO a_expr
 * - a_expr SIMILAR TO a_expr ESCAPE a_expr
 * - a_expr NOT_LA SIMILAR TO a_expr
 * - a_expr NOT_LA SIMILAR TO a_expr ESCAPE a_expr
 *
 * 8. NULL関連
 * - a_expr IS NULL_P
 * - a_expr ISNULL
 * - a_expr IS NOT NULL_P
 * - a_expr NOTNULL
 *
 * 9. 真偽値関連
 * - a_expr IS TRUE_P
 * - a_expr IS NOT TRUE_P
 * - a_expr IS FALSE_P
 * - a_expr IS NOT FALSE_P
 * - a_expr IS UNKNOWN
 * - a_expr IS NOT UNKNOWN
 *
 * 10. 範囲・集合関連
 * - a_expr BETWEEN opt_asymmetric b_expr AND a_expr
 * - a_expr NOT_LA BETWEEN opt_asymmetric b_expr AND a_expr
 * - a_expr BETWEEN SYMMETRIC b_expr AND a_expr
 * - a_expr NOT_LA BETWEEN SYMMETRIC b_expr AND a_expr
 * - a_expr IN_P in_expr
 * - a_expr NOT_LA IN_P in_expr
 * - a_expr row OVERLAPS row
 *
 * 11. サブクエリ関連
 * - a_expr subquery_Op sub_type select_with_parens
 * - a_expr subquery_Op sub_type '(' a_expr ')'
 * - UNIQUE opt_unique_null_treatment select_with_parens
 *
 * 12. ドキュメント・正規化・JSON関連
 * - a_expr IS DOCUMENT_P
 * - a_expr IS NOT DOCUMENT_P
 * - a_expr IS NORMALIZED
 * - a_expr IS unicode_normal_form NORMALIZED
 * - a_expr IS NOT NORMALIZED
 * - a_expr IS NOT unicode_normal_form NORMALIZED
 * - a_expr IS json_predicate_type_constraint json_key_uniqueness_constraint_opt
 * - a_expr IS NOT json_predicate_type_constraint json_key_uniqueness_constraint_opt
 */

impl Visitor {
    // 呼び出した後、cursorは a_expr を指している
    pub(crate) fn visit_a_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // cursor -> c_expr | DEFAULT | Plus | Minus | NOT | qual_Op | a_expr | UNIQUE
        let expr = match cursor.node().kind() {
            SyntaxKind::c_expr => self.visit_c_expr(cursor, src)?,
            SyntaxKind::DEFAULT => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_a_expr(): DEFAULT is not implemented".to_string(),
                ))
            }
            // Unary Expression
            SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::NOT | SyntaxKind::qual_Op => {
                // op a_expr

                // cursor -> op
                let operator = cursor.node().text();
                let mut loc = Location::from(cursor.node().range());

                cursor.goto_next_sibling();
                // cursor -> a_expr

                let operand = self.visit_a_expr(cursor, src)?;
                loc.append(operand.loc());

                Expr::Unary(Box::new(UnaryExpr::new(operator, operand, loc)))
            }
            SyntaxKind::NOT_LA => {
                // NOT キーワードのうち、 NOT LIKE, NOT ILIKE, NOT SIMILAR TO 等のケース
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_a_expr(): Not_LA is not implemented".to_string(),
                ));
            }
            SyntaxKind::a_expr => {
                // cursor -> a_expr
                let expr = self.visit_a_expr(cursor, src)?;

                cursor.goto_next_sibling();
                match cursor.node().kind() {
                    // 算術
                    SyntaxKind::Plus
                    | SyntaxKind::Minus
                    | SyntaxKind::Star
                    | SyntaxKind::Slash
                    | SyntaxKind::Percent
                    | SyntaxKind::Caret => {
                        let lhs = expr;

                        // cursor -> op
                        let op_node = cursor.node();

                        // cursor -> a_expr
                        cursor.goto_next_sibling();
                        let rhs = self.visit_a_expr(cursor, src)?;

                        // 演算子を PrimaryExpr として扱う
                        let op = PrimaryExpr::with_pg_node(op_node, PrimaryExprKind::Expr)?;

                        // ExprSeq として返す
                        let seq = ExprSeq::new(&[lhs, op.into(), rhs]);
                        Expr::ExprSeq(Box::new(seq))
                    }
                    // 論理
                    SyntaxKind::AND | SyntaxKind::OR => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): Operator {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // 比較
                    SyntaxKind::Less
                    | SyntaxKind::Greater
                    | SyntaxKind::Equals
                    | SyntaxKind::LESS_EQUALS
                    | SyntaxKind::GREATER_EQUALS
                    | SyntaxKind::NOT_EQUALS => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): Operator {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // その他の演算子
                    SyntaxKind::qual_Op => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): Operator {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // 型変換
                    SyntaxKind::TYPECAST => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // 属性関連
                    SyntaxKind::COLLATE | SyntaxKind::AT => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // パターンマッチング
                    SyntaxKind::LIKE | SyntaxKind::ILIKE | SyntaxKind::SIMILAR => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // NULL
                    SyntaxKind::IS | SyntaxKind::ISNULL | SyntaxKind::NOTNULL => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // 範囲・集合
                    SyntaxKind::BETWEEN | SyntaxKind::IN_P => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // サブクエリ
                    SyntaxKind::subquery_Op => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    _ => {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_a_expr(): Unexpected syntax. node: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
            }
            SyntaxKind::row => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::UNIQUE => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_a_expr(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;

        Ok(expr)
    }
}
