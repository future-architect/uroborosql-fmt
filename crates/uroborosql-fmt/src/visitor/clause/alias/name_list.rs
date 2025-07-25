use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        table_function_alias::name_list::{Name, ParenthesizedNameList},
        Comment, Location, PrimaryExpr, PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};

impl Visitor {
    pub(crate) fn visit_opt_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenthesizedNameList, UroboroSQLFmtError> {
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
    pub(crate) fn handle_parenthesized_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenthesizedNameList, UroboroSQLFmtError> {
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
        let mut names = self.visit_name_list(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> comment?

        if cursor.node().is_comment() {
            // 行末コメントを想定する
            let comment = Comment::new(cursor.node());

            // exprs は必ず1つ以上要素を持っている
            let last = names.last_mut().unwrap();
            if last.loc().is_same_line(&comment.loc()) {
                last.set_trailing_comment(comment);
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

        Ok(ParenthesizedNameList::new(names, loc, start_comments))
    }

    fn visit_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Name>, UroboroSQLFmtError> {
        // name_list
        // - name ( ',' name)*
        //
        // name: ColId

        cursor.goto_first_child();
        // cursor -> name

        let mut names = vec![];

        ensure_kind!(cursor, SyntaxKind::name, src);
        let first = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;
        names.push(Name::new(first));

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::name => {
                    let name = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;
                    names.push(Name::new(name));
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());

                    // names は必ず1つ以上要素を持っている
                    let last = names.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment);
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
