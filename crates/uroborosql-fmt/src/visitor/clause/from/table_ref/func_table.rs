use crate::{
    config::CONFIG,
    cst::{
        table_function_alias::TableFuncAlias, AlignedExpr, ColumnList, Comment, Expr,
        FunctionTable, Location, PrimaryExpr, PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    util::{convert_identifier_case, convert_keyword_case},
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

impl Visitor {
    /// 呼出し後、cursor は func_table を指している
    pub(crate) fn visit_func_table(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionTable, UroboroSQLFmtError> {
        // func_table
        // - func_expr_windowless opt_ordinality
        // - ROWS FROM '(' rowsfrom_list ')' opt_ordinality

        let loc = cursor.node().range().into();

        cursor.goto_first_child();

        let func_table = match cursor.node().kind() {
            SyntaxKind::func_expr_windowless => {
                let func_expr = self.visit_func_expr_windowless(cursor, src)?;

                cursor.goto_next_sibling();

                // cursor -> opt_ordinality?
                let with_ordinality = if cursor.node().kind() == SyntaxKind::opt_ordinality {
                    Some(self.visit_opt_ordinality(cursor)?)
                } else {
                    None
                };

                FunctionTable::new(func_expr, with_ordinality, loc)
            }
            SyntaxKind::ROWS => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_table(): ROWS node appeared. 'ROWS FROM (rowsfrom_list)' pattern is not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_table(): unexpected node appeared. node: {}\n{}",
                    cursor.node().kind(),
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_table, src);

        Ok(func_table)
    }

    fn visit_opt_ordinality(
        &mut self,
        cursor: &mut TreeCursor,
    ) -> Result<String, UroboroSQLFmtError> {
        // opt_ordinality:
        // - WITH_LA ORDINALITY

        let text = cursor.node().text();
        Ok(convert_keyword_case(text))
    }

    /// func_alias_clause を visit し、 as キーワード (Option) と Expr を返す
    /// 呼出し後、cursor は func_alias_clause を指している
    pub(crate) fn visit_func_alias_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<String>, Expr), UroboroSQLFmtError> {
        // func_alias_clause:
        // - alias_clause
        // - AS '(' TableFuncElementList ')'   このケースはASを除去するとパースできないため除去しない
        // - ColId '(' TableFuncElementList ')'
        // - AS ColId '(' TableFuncElementList ')'

        cursor.goto_first_child();

        // alias_clause なら先に処理して early return
        if cursor.node().kind() == SyntaxKind::alias_clause {
            let alias_clause = self.visit_alias_clause(cursor, src)?;

            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::func_alias_clause, src);

            return Ok(alias_clause);
        }

        // cursor -> AS?
        let optional_as = if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = convert_keyword_case(cursor.node().text());
            cursor.goto_next_sibling();

            Some(as_keyword)
        } else {
            None
        };

        // cursor -> ColId?
        let mut alias_loc = Location::from(cursor.node().range());
        let optional_colid = if cursor.node().kind() == SyntaxKind::ColId {
            let col_id = convert_identifier_case(cursor.node().text());

            cursor.goto_next_sibling();

            Some(col_id.to_string())
        } else {
            None
        };

        // AS がある場合、remove_table_as_keyword の値に応じてASを除去する
        // このとき、ColId が無いケースで AS を除去すると不正な構文になる。そのため ColId が無い場合はオプションの設定値にかかわらず AS を除去しない
        //
        // ColId があるケース：
        // OK: select * from unnest(a) as t(id int, name text);
        // OK: select * from unnest(a)    t(id int, name text);
        //
        // ColId が無いケース：
        // OK: select * from unnest(a) as (id int, name text)
        // NG: select * from unnest(a)    (id int, name text)
        //                             ^^^ ここでパースエラー
        //
        let (as_keyword, col_id) = if let Some(as_keyword) = optional_as {
            // ColId が存在して、remove_table_as_keyword が true の場合、AS を除去する
            if optional_colid.is_some() && CONFIG.read().unwrap().remove_table_as_keyword {
                (None, optional_colid)
            } else {
                (Some(convert_keyword_case(&as_keyword)), optional_colid)
            }
        } else {
            (None, optional_colid)
        };

        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);
        let table_func_element_list =
            self.handle_parenthesized_table_func_element_list(cursor, src)?;

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        alias_loc.append(Location::from(cursor.node().range()));

        let func_alias = TableFuncAlias::new(col_id, table_func_element_list, alias_loc);
        let expr = Expr::TableFuncAlias(Box::new(func_alias));

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_alias_clause, src);

        Ok((as_keyword, expr))
    }

    /// 括弧で囲まれた TableFuncElementList を走査する
    /// 呼出し時、cursor は '(' を指している
    /// 呼出し後、cursor は ')' を指している
    fn handle_parenthesized_table_func_element_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // '(' TableFuncElementList ')'
        // ^^^                      ^^^
        // 呼出し時                  呼出し後

        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> comment?

        // 開き括弧と式との間にあるコメントを保持
        let mut start_comments = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            start_comments.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> TableFuncElementList
        ensure_kind!(cursor, SyntaxKind::TableFuncElementList, src);
        let mut exprs = self.visit_table_func_element_list(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> comment?

        if cursor.node().is_comment() {
            // 行末コメントを想定する
            let comment = Comment::new(cursor.node());

            // exprs は必ず1つ以上要素を持っている
            let last = exprs.last_mut().unwrap();
            if last.loc().is_same_line(&comment.loc()) {
                last.set_trailing_comment(comment)?;
            } else {
                // 行末コメント以外のコメントは想定していない
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_parenthesized_table_func_element_list(): Unexpected comment\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        loc.append(Location::from(cursor.node().range()));

        Ok(ColumnList::new(exprs, loc, start_comments))
    }

    fn visit_table_func_element_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // TableFuncElementList:
        // - TableFuncElement ( ',' TableFuncElementList )*
        //
        // this node is flatten: https://github.com/future-architect/postgresql-cst-parser/pull/29

        cursor.goto_first_child();
        // cursor -> TableFuncElement

        let mut exprs = vec![];

        ensure_kind!(cursor, SyntaxKind::TableFuncElement, src);
        let first = self.visit_table_func_element(cursor, src)?;
        exprs.push(first);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::TableFuncElement => {
                    let expr = self.visit_table_func_element(cursor, src)?;
                    exprs.push(expr);
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());

                    // exprs は必ず1つ以上要素を持っている
                    let last = exprs.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_table_func_element_list(): Unexpected comment\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_table_func_element_list(): unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::TableFuncElementList, src);

        Ok(exprs)
    }

    fn visit_table_func_element(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // TableFuncElement:
        // - ColId Typename opt_collate_clause?

        cursor.goto_first_child();

        // cursor -> ColId
        ensure_kind!(cursor, SyntaxKind::ColId, src);
        let col_id = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;

        let mut aligned = Expr::Primary(Box::new(col_id)).to_aligned();

        cursor.goto_next_sibling();
        // cursor -> Typename

        ensure_kind!(cursor, SyntaxKind::Typename, src);
        let typename = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;
        aligned.add_rhs(None, typename.into());

        cursor.goto_next_sibling();
        // cursor -> opt_collate_clause?

        if cursor.node().kind() == SyntaxKind::opt_collate_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_table_func_element(): collate clause in function alias is not implemented yet.\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();
        // cursor -> TableFuncElement
        ensure_kind!(cursor, SyntaxKind::TableFuncElement, src);

        Ok(aligned)
    }
}
