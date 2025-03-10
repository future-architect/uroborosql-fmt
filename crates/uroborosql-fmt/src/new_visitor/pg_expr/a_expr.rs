use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        unary::UnaryExpr, AlignedExpr, ColumnList, Comment, Expr, ExprSeq, Location, PrimaryExpr,
        PrimaryExprKind, SeparatedLines,
    },
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    CONFIG,
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

            SyntaxKind::a_expr => {
                // cursor -> a_expr
                let expr = self.visit_a_expr(cursor, src)?;

                // a_expr の 子供が a_expr のケース
                if !cursor.goto_next_sibling() {
                    cursor.goto_parent();
                    return Ok(expr);
                }

                // cursor -> 算術演算子 | 比較演算子 | 論理演算子 | 型変換 | 属性関連 | パターンマッチング | IS | ISNULL | NOTNULL | IN | サブクエリ
                match cursor.node().kind() {
                    // 算術演算: ExprSeq
                    SyntaxKind::Plus
                    | SyntaxKind::Minus
                    | SyntaxKind::Star
                    | SyntaxKind::Slash
                    | SyntaxKind::Percent
                    | SyntaxKind::Caret => {
                        // 二項算術演算子
                        // - a_expr '+' a_expr
                        // - a_expr '-' a_expr
                        // - a_expr '*' a_expr
                        // - a_expr '/' a_expr
                        // - a_expr '%' a_expr
                        // - a_expr '^' a_expr
                        //   [lhs]  [op]  [rhs]
                        //          ^^^^ current node
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
                    // 比較演算: AlignedExpr
                    SyntaxKind::Less
                    | SyntaxKind::Greater
                    | SyntaxKind::Equals
                    | SyntaxKind::LESS_EQUALS
                    | SyntaxKind::GREATER_EQUALS
                    | SyntaxKind::NOT_EQUALS
                    | SyntaxKind::qual_Op => {
                        let lhs = expr;

                        // cursor -> op
                        let op_node = cursor.node();

                        // unify_not_equalがtrueの場合は <> を != に統一する
                        let op_str =
                            if CONFIG.read().unwrap().unify_not_equal && op_node.text() == "<>" {
                                "!=".to_string()
                            } else {
                                op_node.text().to_string()
                            };

                        // cursor -> a_expr
                        cursor.goto_next_sibling();
                        let rhs = self.visit_a_expr(cursor, src)?;

                        let mut aligned = AlignedExpr::new(lhs);
                        aligned.add_rhs(Some(op_str), rhs);

                        Expr::Aligned(Box::new(aligned))
                    }
                    // 論理
                    SyntaxKind::AND | SyntaxKind::OR => {
                        let mut boolean_expr = SeparatedLines::new();

                        let lhs = expr;

                        // 左辺がBooleanの場合は初期化したBooleanExprを左辺で上書き
                        match lhs {
                            Expr::Boolean(boolean) => boolean_expr = *boolean,
                            _ => boolean_expr.add_expr(lhs.to_aligned(), None, vec![]),
                        }

                        // cursor -> COMMENT | op

                        while cursor.node().is_comment() {
                            boolean_expr.add_comment_to_child(Comment::pg_new(cursor.node()))?;
                            cursor.goto_next_sibling();
                        }

                        let sep = convert_keyword_case(cursor.node().text());

                        cursor.goto_next_sibling();
                        // cursor -> _expression

                        let mut comments = vec![];
                        while cursor.node().is_comment() {
                            comments.push(Comment::pg_new(cursor.node()));
                            cursor.goto_next_sibling();
                        }

                        let right = self.visit_a_expr(cursor, src)?;

                        if let Expr::Boolean(boolean) = right {
                            // 右辺がbooleanの場合はマージ処理を行う
                            boolean_expr.merge_boolean_expr(sep, *boolean);
                        } else {
                            boolean_expr.add_expr(right.to_aligned(), Some(sep), comments);
                        }

                        Expr::Boolean(Box::new(boolean_expr))
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
                    // IS: AlignedExpr
                    SyntaxKind::IS => {
                        let lhs = expr;
                        // cursor -> IS
                        let op = convert_keyword_case(cursor.node().text());

                        cursor.goto_next_sibling();
                        // cursor -> NOT?
                        let not_node = if cursor.node().kind() == SyntaxKind::NOT {
                            let not_node = cursor.node();
                            cursor.goto_next_sibling();

                            Some(not_node)
                        } else {
                            None
                        };

                        // cursor -> NULL_P | TRUE_P | FALSE_P | DISTINCT | UNKNOWN | DOCUMENT_P | NORMALIZED | unicode_normal_form | json_predicate_type_constraint
                        let last_expr = match cursor.node().kind() {
                            SyntaxKind::NULL_P | SyntaxKind::TRUE_P | SyntaxKind::FALSE_P => {
                                // IS NOT? NULL_P
                                // IS NOT? TRUE_P
                                // IS NOT? FALSE_P

                                let primary = PrimaryExpr::with_pg_node(
                                    cursor.node(),
                                    PrimaryExprKind::Keyword,
                                )?;
                                Expr::Primary(Box::new(primary))
                            }
                            SyntaxKind::DISTINCT => {
                                // IS NOT? DISTINCT FROM

                                return Err(UroboroSQLFmtError::Unimplemented(format!(
                                    "visit_a_expr(): {} is not implemented.\n{}",
                                    cursor.node().kind(),
                                    pg_error_annotation_from_cursor(cursor, src)
                                )));
                            }
                            SyntaxKind::UNKNOWN => {
                                // IS NOT? UNKNOWN

                                return Err(UroboroSQLFmtError::Unimplemented(format!(
                                    "visit_a_expr(): {} is not implemented.\n{}",
                                    cursor.node().kind(),
                                    pg_error_annotation_from_cursor(cursor, src)
                                )));
                            }
                            SyntaxKind::DOCUMENT_P
                            | SyntaxKind::NORMALIZED
                            | SyntaxKind::unicode_normal_form
                            | SyntaxKind::json_predicate_type_constraint => {
                                // - IS NOT? DOCUMENT_P
                                // - IS NOT? unicode_normal_form? NORMALIZED
                                // - IS NOT? json_predicate_type_constraint json_key_uniqueness_constraint_opt

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

                        // AlignedExpr の右辺
                        // NOT がある場合は NOT を演算子とした UnaryExpr にする
                        let rhs = if let Some(not_node) = not_node {
                            let mut loc = Location::from(not_node.range());
                            loc.append(last_expr.loc());

                            let unary = UnaryExpr::new(not_node.text(), last_expr, loc);
                            Expr::Unary(Box::new(unary))
                        } else {
                            last_expr
                        };

                        // IS を演算子とした AlignedExpr
                        let mut aligned = AlignedExpr::new(lhs);
                        aligned.add_rhs(Some(op), rhs);

                        Expr::Aligned(Box::new(aligned))
                    }
                    SyntaxKind::ISNULL | SyntaxKind::NOTNULL => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_a_expr(): {} is not implemented.\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                    // IN: AlignedExpr
                    SyntaxKind::IN_P => {
                        // IN_P in_expr

                        let aligned =
                            self.visit_flat_in_keyword_and_in_expr(cursor, src, expr, None)?;
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
                                // NOT_LA IN
                                let aligned = self.visit_flat_in_keyword_and_in_expr(
                                    cursor,
                                    src,
                                    expr,
                                    Some(not_text),
                                )?;
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
            SyntaxKind::NOT_LA => {
                // `NOT_LA a_expr`` が a_expr の定義に含まれるが、 NOT_LA はその後に BETWEEN, IN, LIKE, ILIKE, SIMILAR のいずれかが続く場合にしか生成されない
                // そして、それらのキーワードは `a_expr NOT_LA <keyword> <...>` のようなケースでしか出現しないため、この位置に NOT_LA が現れることはない
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_a_expr(): Unexpected syntax. node: {}\n{}",
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

    /// Expr と NOT キーワード を受け取り、 `IN_P in_expr` を走査して AlignedExpr に変換する
    /// a_expr の子が `IN_P in_expr` の場合と `NOT_LA IN_P in_expr` の場合があり、両者の処理を共通化するために利用
    /// 呼出し時、 cursor は IN_P を指している
    /// 呼出し後、cursor は in_expr （同階層の最後の要素）を指している
    fn visit_flat_in_keyword_and_in_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
        not_keyword: Option<&str>,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
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
        // - select_with_parens
        // - '(' expr_list ')'

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
        // cursor -> a_expr

        let first_expr = self.visit_a_expr(cursor, src)?;

        let mut exprs = vec![first_expr.to_aligned()];

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
