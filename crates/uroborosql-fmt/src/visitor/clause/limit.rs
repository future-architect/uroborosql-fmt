use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    /// LIMIT句をClause構造体で返す
    /// SELECT文で使用する
    pub(crate) fn visit_limit_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();
        ensure_kind(cursor, "LIMIT")?;
        let mut limit_clause = Clause::from_node(cursor.node(), src);

        cursor.goto_next_sibling();
        // cursor -> number | ALL

        match cursor.node().kind() {
            "number" => {
                // numberをExprに格納
                let number = self.visit_expr(cursor, src)?;
                // numberからBody::SingleLineを作成
                let body = Body::SingleLine(Box::new(SingleLine::new(number)));
                limit_clause.set_body(body);
            }
            "ALL" => {
                // "LIMIT ALL"というキーワードと捉えて構造体に格納
                limit_clause.extend_kw_with_tab(cursor.node(), src);
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    r#"visit_limit_clause(): expected node is number or ALL, but actual {}\n{:#?}"#,
                    cursor.node().kind(),
                    cursor.node().range()
                )));
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "limit_clause")?;

        Ok(limit_clause)
    }
}
