use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{cst::Statement, error::UroboroSQLFmtError, NewVisitor as Visitor};

impl Visitor {
    pub(crate) fn visit_update_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        unimplemented!()
    }
}
