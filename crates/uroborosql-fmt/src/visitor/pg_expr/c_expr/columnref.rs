use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AsteriskExpr, Expr, PrimaryExpr},
    error::UroboroSQLFmtError,
    util::convert_identifier_case,
    visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
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

        pg_ensure_kind!(cursor, SyntaxKind::ColId, src);
        let mut columnref_text = cursor.node().text().to_string();

        if cursor.goto_next_sibling() {
            // cursor -> indirection
            pg_ensure_kind!(cursor, SyntaxKind::indirection, src);

            // indirection
            // - indirection_el
            //    - `.` attr_name
            //    - `.` `*`
            //    - `[` a_expr `]`
            //    - `[` opt_slice_bound `:` opt_slice_bound `]`
            //
            // indirection はフラット化されている: https://github.com/future-architect/postgresql-cst-parser/pull/7

            let indirection_text = cursor.node().text();

            // 配列アクセスは unimplemented
            if indirection_text.contains('[') {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_columnref(): array access is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            // indirection にあたるテキストから空白文字を除去し、そのまま追加している
            let whitespace_removed = indirection_text
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            columnref_text.push_str(&whitespace_removed);
        }

        // アスタリスクが含まれる場合はAsteriskExprに変換する
        let expr = if columnref_text.contains('*') {
            AsteriskExpr::new(
                convert_identifier_case(&columnref_text),
                cursor.node().range().into(),
            )
            .into()
        } else {
            PrimaryExpr::new(
                convert_identifier_case(&columnref_text),
                cursor.node().range().into(),
            )
            .into()
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::columnref, src);

        Ok(expr)
    }
}
