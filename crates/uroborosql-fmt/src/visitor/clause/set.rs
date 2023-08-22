use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor, COMMA, COMMENT},
};

impl Visitor {
    /// SET句をClause構造体で返す
    /// UPDATE文、INSERT文で使用する
    pub(crate) fn visit_set_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        ensure_kind(cursor, "SET")?;
        let mut set_clause = Clause::from_node(cursor.node(), src);
        cursor.goto_next_sibling();

        ensure_kind(cursor, "set_clause_body")?;
        cursor.goto_first_child();

        let mut sep_lines = SeparatedLines::new();

        // commaSep1(set_clause_item)
        let aligned = self.visit_set_clause_item(cursor, src)?;
        sep_lines.add_expr(aligned, None, vec![]);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    sep_lines.add_comment_to_child(comment)?;
                }
                COMMA => continue,
                _ => {
                    let aligned = self.visit_set_clause_item(cursor, src)?;
                    sep_lines.add_expr(aligned, Some(COMMA.to_string()), vec![]);
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "set_clause_body")?;

        // set_clauseにBodyをセット
        set_clause.set_body(Body::SepLines(sep_lines));

        cursor.goto_parent();
        ensure_kind(cursor, "set_clause")?;

        Ok(set_clause)
    }

    fn visit_set_clause_item(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        if cursor.node().kind() == "assigment_expression" {
            // tree-sitter-sqlのタイポでnが抜けている点に注意
            let aligned = self.visit_assign_expr(cursor, src)?;
            Ok(aligned)
        } else if cursor.node().kind() == "(" {
            let lhs = Expr::ColumnList(Box::new(self.visit_column_list(cursor, src)?));

            cursor.goto_next_sibling();
            ensure_kind(cursor, "=")?;

            cursor.goto_next_sibling();

            let rhs = if cursor.node().kind() == "select_subexpression" {
                Expr::Sub(Box::new(self.visit_select_subexpr(cursor, src)?))
            } else {
                Expr::ColumnList(Box::new(self.visit_column_list(cursor, src)?))
            };

            let mut aligned = AlignedExpr::new(lhs);
            aligned.add_rhs(Some("=".to_string()), rhs);

            Ok(aligned)
        } else {
            Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                r#"visit_set_clause(): expected node is assigment_expression, "(" or select_subexpression, but actual {}\n{:#?}"#,
                cursor.node().kind(),
                cursor.node().range()
            )))
        }
    }
}
