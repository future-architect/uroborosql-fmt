use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    config::CONFIG,
    cst::{
        table_function_alias::TableFuncAlias, AlignedExpr, ColumnList, Comment, Expr, Location,
        PrimaryExpr, PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    util::convert_identifier_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// alias_clause を visit し、 as キーワード (Option) と Expr を返す
    /// 呼出し後、cursor は alias_clause を指している
    pub(crate) fn visit_alias_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<String>, Expr), UroboroSQLFmtError> {
        // alias_clause
        // - [AS] ColId ['(' name_list ')']

        // cursor -> alias_clause
        ensure_kind!(cursor, SyntaxKind::alias_clause, src);

        cursor.goto_first_child();
        // cursor -> AS?
        let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = cursor.node().text().to_string();
            cursor.goto_next_sibling();

            // remove_table_as_keyword が有効ならば AS を除去
            if CONFIG.read().unwrap().remove_table_as_keyword {
                None
            } else {
                Some(as_keyword)
            }
        } else {
            None
        };

        // cursor -> ColId
        let col_id = convert_identifier_case(cursor.node().text());
        let col_id_loc = Location::from(cursor.node().range());
        cursor.goto_next_sibling();

        // cursor -> '('?
        let expr = if cursor.node().kind() == SyntaxKind::LParen {
            let list = self.handle_parenthesized_name_list(cursor, src)?;
            let table_func_alias = TableFuncAlias::new(Some(col_id), list, col_id_loc);

            Expr::TableFuncAlias(Box::new(table_func_alias))
        } else {
            let primary = PrimaryExpr::new(col_id, col_id_loc);
            Expr::Primary(Box::new(primary))
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::alias_clause, src);

        Ok((as_keyword, expr))
    }

    pub(crate) fn visit_opt_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // opt_name_list
        // - '(' name_list ')'

        cursor.goto_first_child();

        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);

        let column_list = self.handle_parenthesized_name_list(cursor, src)?;

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::opt_name_list, src);

        Ok(column_list)
    }

    /// 括弧で囲まれた name_list を走査する
    /// 呼出し時、cursor は '(' を指している
    /// 呼出し後、cursor は ')' を指している
    fn handle_parenthesized_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // '(' name_list ')'

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

        // cursor -> name_list
        ensure_kind!(cursor, SyntaxKind::name_list, src);
        let mut exprs = self.visit_name_list(cursor, src)?;

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
                    "handle_parenthesized_name_list(): Unexpected comment\n{}",
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

    fn visit_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // name_list
        // - name ( ',' name)*
        //
        // name: ColId

        cursor.goto_first_child();
        // cursor -> name

        let mut names = vec![];

        ensure_kind!(cursor, SyntaxKind::name, src);
        let first = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;
        names.push(Expr::Primary(Box::new(first)).to_aligned());

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::name => {
                    let name = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;
                    names.push(Expr::Primary(Box::new(name)).to_aligned());
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());

                    // names は必ず1つ以上要素を持っている
                    let last = names.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_name_list(): Unexpected comment\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_name_list: unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::name_list, src);

        Ok(names)
    }
}
