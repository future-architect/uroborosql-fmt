use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AsteriskExpr, Comment, Expr, Location, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    util::convert_identifier_case,
    visitor::{ensure_kind, error_annotation_from_cursor},
};

use super::Visitor;
impl Visitor {
    pub fn visit_columnref(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // columnref
        // - ColId
        // - ColId indirection
        //   - e.g.: `a.field`, `a.field[1]`

        // cursor -> ColId (必ず存在する)
        cursor.goto_first_child();

        ensure_kind!(cursor, SyntaxKind::ColId, src);
        let mut columnref_text = convert_identifier_case(cursor.node().text());

        if cursor.goto_next_sibling() {
            // cursor -> indirection
            ensure_kind!(cursor, SyntaxKind::indirection, src);

            // indirection
            // - indirection_el
            //    - `.` attr_name
            //    - `.` `*`
            //    - `[` a_expr `]`
            //    - `[` opt_slice_bound `:` opt_slice_bound `]`
            //
            // indirection はフラット化されている: https://github.com/future-architect/postgresql-cst-parser/pull/7

            cursor.goto_first_child();

            loop {
                ensure_kind!(cursor, SyntaxKind::indirection_el, src);

                cursor.goto_first_child();

                match cursor.node().kind() {
                    SyntaxKind::Dot => {
                        //    - `.` attr_name
                        //    - `.` `*`
                        columnref_text.push('.');

                        cursor.goto_next_sibling();

                        let comment = if cursor.node().is_comment() {
                            let comment = Comment::new(cursor.node());
                            cursor.goto_next_sibling();
                            Some(comment)
                        } else {
                            None
                        };

                        if let Some(comment) = comment {
                            if !comment.is_block_comment()
                                || !comment
                                    .loc()
                                    .is_next_to(&Location::from(cursor.node().range()))
                            {
                                // コメントが置換文字列ではない場合はエラー
                                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                    "visit_columnref(): unexpected comment node appeared.\n{}",
                                    error_annotation_from_cursor(cursor, src)
                                )));
                            }

                            columnref_text.push_str(comment.text());
                        }

                        columnref_text.push_str(&convert_identifier_case(cursor.node().text()));
                    }
                    _ => {
                        // 配列アクセスは unimplemented
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_columnref(): array access is not implemented\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }

                cursor.goto_parent();

                if !cursor.goto_next_sibling() {
                    break;
                }
            }

            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::indirection, src);
        }

        // アスタリスクが含まれる場合はAsteriskExprに変換する
        let expr = if columnref_text == "*" || columnref_text.contains(".*") {
            AsteriskExpr::new(&columnref_text, cursor.node().range().into()).into()
        } else {
            PrimaryExpr::new(&columnref_text, cursor.node().range().into()).into()
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::columnref, src);

        Ok(expr)
    }
}
