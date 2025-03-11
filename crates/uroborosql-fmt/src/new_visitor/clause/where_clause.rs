use postgresql_cst_parser::syntax_kind::SyntaxKind;
use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{create_clause, ensure_kind, pg_create_clause, pg_ensure_kind, Visitor},
};

impl Visitor {
    pub(crate) fn visit_where_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // where_clauseは必ずWHEREを子供に持つ
        cursor.goto_first_child();

        // cursor -> WHERE
        let mut clause = create_clause(cursor, src, "WHERE")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> _expression
        let expr = self.visit_expr(cursor, src)?;

        // 結果として得られた式をBodyに変換する
        let body = Body::from(expr);

        clause.set_body(body);

        // cursorをwhere_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "where_clause", src)?;

        Ok(clause)
    }

    pub(crate) fn pg_visit_where_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // cursor -> WHERE
        let mut clause = pg_create_clause(cursor, SyntaxKind::WHERE)?;
        cursor.goto_next_sibling();
        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> a_expr
        let expr = self.visit_a_expr(cursor, src)?;

        // 結果として得られた式をBodyに変換する
        let body = Body::from(expr);

        clause.set_body(body);

        // cursorをwhere_clauseに戻す
        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::where_clause, src)?;

        Ok(clause)
    }
}
