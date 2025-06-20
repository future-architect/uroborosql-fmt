use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, Visitor},
};

impl Visitor {
    /// キーワードのみで構成される関数をフォーマットする
    pub(crate) fn handle_only_keyword_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        keyword_kind: SyntaxKind,
    ) -> Result<PrimaryExpr, UroboroSQLFmtError> {
        pg_ensure_kind!(cursor, expr: keyword_kind, src);

        // cursor -> keyword
        let keyword = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;

        Ok(keyword)
    }
}
