mod element_list;

use crate::{
    config::CONFIG,
    cst::{table_function_alias::TableFuncAlias, ColumnList, Expr, FunctionTable, Location},
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

                // cursor -> comment?
                if cursor.node().is_comment() {
                    // unimplemented
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_func_table(): comment before opt_ordinality appeared. comment at this position is not supported yet.\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                // cursor -> opt_ordinality?
                let with_ordinality = if cursor.node().kind() == SyntaxKind::opt_ordinality {
                    Some(self.visit_opt_ordinality(cursor, src)?)
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
        src: &str,
    ) -> Result<String, UroboroSQLFmtError> {
        // opt_ordinality:
        // - WITH_LA ORDINALITY

        cursor.goto_first_child();

        // cursor -> WITH_LA
        ensure_kind!(cursor, SyntaxKind::WITH_LA, src);
        let with = convert_keyword_case(cursor.node().text());
        cursor.goto_next_sibling();

        // cursor -> ORDINALITY
        ensure_kind!(cursor, SyntaxKind::ORDINALITY, src);
        let ordinality = convert_keyword_case(cursor.node().text());

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::opt_ordinality, src);

        let text = [with, ordinality].join(" ");
        Ok(text)
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
        let col_id = if cursor.node().kind() == SyntaxKind::ColId {
            let text = convert_identifier_case(cursor.node().text());

            cursor.goto_next_sibling();
            Some(text)
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
        let as_keyword_to_render = optional_as.filter(|_| {
            // 以下の場合にASを除去せず残す：
            // - ColIdが無い場合（構文上の制約）
            // - remove_table_as_keyword が false の場合（フォーマットオプションに基づく挙動）
            // それ以外の場合はASを除去する
            col_id.is_none() || !CONFIG.read().unwrap().remove_table_as_keyword
        });

        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);
        let parenthesized_list = self.handle_parenthesized_table_func_element_list(cursor, src)?;

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        // alias_loc が示す位置を閉じ括弧の位置までに更新
        alias_loc.append(Location::from(cursor.node().range()));

        let column_list = ColumnList::from(parenthesized_list);
        let func_alias = TableFuncAlias::new(col_id, column_list, alias_loc);
        let expr = Expr::TableFuncAlias(Box::new(func_alias));

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_alias_clause, src);

        Ok((as_keyword_to_render, expr))
    }
}
