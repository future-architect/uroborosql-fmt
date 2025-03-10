mod arithmetic;
mod comparison;
mod in_expr;
mod is_expr;
mod logical;
mod unary;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{cst::Expr, error::UroboroSQLFmtError};

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
    /// 呼び出した後、cursorは a_expr を指している
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
                self.handle_unary_expr_nodes(cursor, src)?
            }
            SyntaxKind::a_expr => {
                // cursor -> a_expr
                let expr = self.visit_a_expr(cursor, src)?;

                // a_expr の 子供が 単一の a_expr のケース
                if !cursor.goto_next_sibling() {
                    cursor.goto_parent();
                    return Ok(expr);
                }

                // cursor -> 算術演算子 | 比較演算子 | 論理演算子 | TYPECAST | COLLATE | AT | LIKE | ILIKE | SIMILAR | IS | ISNULL | NOTNULL | IN | サブクエリ
                match cursor.node().kind() {
                    // 算術演算
                    SyntaxKind::Plus
                    | SyntaxKind::Minus
                    | SyntaxKind::Star
                    | SyntaxKind::Slash
                    | SyntaxKind::Percent
                    | SyntaxKind::Caret => {
                        self.handle_arithmetic_binary_expr_nodes(cursor, src, expr)?
                    }
                    // 比較演算
                    SyntaxKind::Less
                    | SyntaxKind::Greater
                    | SyntaxKind::Equals
                    | SyntaxKind::LESS_EQUALS
                    | SyntaxKind::GREATER_EQUALS
                    | SyntaxKind::NOT_EQUALS
                    | SyntaxKind::qual_Op => {
                        self.handle_comparison_expr_nodes(cursor, src, expr)?
                    }
                    // 論理
                    SyntaxKind::AND | SyntaxKind::OR => {
                        self.handle_logical_expr_nodes(cursor, src, expr)?
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
                    // IS
                    SyntaxKind::IS => self.handle_is_expr_nodes(cursor, src, expr)?,
                    SyntaxKind::ISNULL | SyntaxKind::NOTNULL => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // IN
                    SyntaxKind::IN_P => {
                        // IN_P in_expr

                        let aligned = self.handle_in_expr_nodes(cursor, src, expr, None)?;
                        Expr::Aligned(Box::new(aligned))
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
                            "visit_a_expr(): {} is not implemented.\n{}",
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
                                    self.handle_in_expr_nodes(cursor, src, expr, Some(not_text))?;
                                Expr::Aligned(Box::new(aligned))
                            }
                            SyntaxKind::LIKE | SyntaxKind::ILIKE | SyntaxKind::SIMILAR => {
                                // NOT_LA (LIKE | ILIKE | SIMILAR)
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
                        }
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

        // cursor -> (last_node)
        cursor.goto_parent();
        // cursor -> a_expr (parent)
        pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;

        Ok(expr)
    }
}
