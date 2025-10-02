use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AsteriskExpr, Comment, Expr, Location, PrimaryExpr},
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

        // ColId、indirectionで出現するノードを大文字小文字変換済みの文字列として順にpushしていく
        // 途中で出現するバインドパラメータは大文字小文字変換を行わずそのまま文字列としてpushする
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

                        if cursor.node().is_comment() {
                            let comment = Comment::new(cursor.node());
                            cursor.goto_next_sibling();

                            if comment.is_block_comment()
                                && comment
                                    .loc()
                                    .is_next_to(&Location::from(cursor.node().range()))
                            {
                                // バインドパラメータの場合
                                columnref_text.push_str(comment.text());
                            } else {
                                // コメントがバインドパラメータではない場合はエラー
                                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                    "visit_columnref(): unexpected comment node appeared.\n{}",
                                    error_annotation_from_cursor(cursor, src)
                                )));
                            }
                        }

                        columnref_text.push_str(&convert_identifier_case(cursor.node().text()));
                    }
                    _ => {
                        //    - `[` a_expr `]`
                        //    - `[` opt_slice_bound `:` opt_slice_bound `]`

                        // 配列アクセスは unimplemented
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_columnref(): array access is not implemented\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }

                cursor.goto_parent();
                ensure_kind!(cursor, SyntaxKind::indirection_el, src);

                if !cursor.goto_next_sibling() {
                    break;
                }
            }

            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::indirection, src);
        }

        // アスタリスクが含まれる場合はAsteriskExprに変換する
        // 単純に*が含まれるかどうかで判定すると、途中にバインドパラメータが含まれる場合もtrueとなるため、「*と完全一致」または「.*が含まれる」の場合にAsteriskExprに変換する
        // また、columnref_textは既にcase変換済みなため、ここでは行わない
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
