mod for_locking;
mod from;
mod group;
mod having;
mod select;
mod sort;
mod where_clause;

use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{Expr, PrimaryExpr},
    error::UroboroSQLFmtError,
    util::convert_identifier_case,
    NewVisitor as Visitor,
};

use super::{pg_ensure_kind, pg_error_annotation_from_cursor};

impl Visitor {
    pub(crate) fn visit_qualified_name(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // qualified_name
        // - ColId
        // - ColId indirection

        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::ColId, src)?;

        let mut qualified_name_text = cursor.node().text().to_string();

        if cursor.goto_next_sibling() {
            // indirection が存在する場合
            pg_ensure_kind(cursor, SyntaxKind::indirection, src)?;

            let indirection_text = cursor.node().text().to_string();

            if indirection_text.contains('[') {
                // この場所での subscript （[1] など）は構文定義上可能だが、PostgreSQL側でrejectされる不正な記述
                // - https://github.com/postgres/postgres/blob/master/src/backend/parser/gram.y#L17303-L17304
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_qualified_name(): invalid subscript notation appeared.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            // 空白を除去してqualified_name_textに追加
            qualified_name_text.push_str(
                &indirection_text
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>(),
            );
        }

        cursor.goto_parent();
        // cursor -> qualified_name
        pg_ensure_kind(cursor, SyntaxKind::qualified_name, src)?;

        let primary = PrimaryExpr::new(
            convert_identifier_case(&qualified_name_text),
            cursor.node().range().into(),
        );

        Ok(primary.into())
    }
}
