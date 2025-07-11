use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Comment, Expr, SeparatedLines},
    error::UroboroSQLFmtError,
    visitor::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor, COMMA},
};

// for_locking_clause
// - for_locking_items
// - FOR READ ONLY
//
// for_locking_items
// - for_locking_item (for_locking_item)*
//
// for_locking_item
// - for_locking_strength locked_rels_list? opt_nowait_or_skip?
//
// for_locking_strength
// - FOR UPDATE
// - FOR NO KEY UPDATE
// - FOR SHARE
// - FOR KEY SHARE
//
// locked_rels_list
// - OF qualified_name_list
//
// opt_nowait_or_skip
// - NOWAIT
// - SKIP LOCKED

impl Visitor {
    /// for_locking_clause を走査して、Clause のベクタを返す
    pub(crate) fn visit_for_locking_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        // for_locking_clause
        // - for_locking_items
        // - FOR READ ONLY

        cursor.goto_first_child();

        let clauses = match cursor.node().kind() {
            SyntaxKind::for_locking_items => self.visit_for_locking_items(cursor, src)?,
            SyntaxKind::FOR => {
                // FOR READ ONLY
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_for_locking_clause: FOR READ ONLY is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_for_locking_clause: unexpected node kind: {}",
                    cursor.node().kind()
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::for_locking_clause, src);

        Ok(clauses)
    }

    fn visit_for_locking_items(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        // for_locking_items
        // - for_locking_item (for_locking_item)*
        let mut clauses = vec![];

        cursor.goto_first_child();

        // first for_locking_item
        self.visit_for_locking_item(cursor, src, &mut clauses)?;

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::for_locking_item => {
                    self.visit_for_locking_item(cursor, src, &mut clauses)?;
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    clauses.last_mut().unwrap().add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_for_locking_items: unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::for_locking_items, src);

        Ok(clauses)
    }

    /// for_locking_item を走査して、Clause をベクタに追加する
    fn visit_for_locking_item(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clauses: &mut Vec<Clause>,
    ) -> Result<(), UroboroSQLFmtError> {
        // for_locking_item
        // - for_locking_strength locked_rels_list? opt_nowait_or_skip?

        // for_locking_strength locked_rels_list までで一つ、 opt_nowait_or_skip で一つの Clause になる

        cursor.goto_first_child();

        // cursor -> for_locking_strength
        let mut for_update_clause = self.visit_for_locking_strength(cursor, src)?;
        cursor.goto_next_sibling();

        // cursor -> locked_rels_list?
        if cursor.node().kind() == SyntaxKind::locked_rels_list {
            self.visit_locked_rels_list(cursor, src, &mut for_update_clause)?;
            cursor.goto_next_sibling();
        }

        // cursor -> comments?
        self.pg_consume_comments_in_clause(cursor, &mut for_update_clause)?;

        clauses.push(for_update_clause);

        // cursor -> opt_nowait_or_skip?
        if cursor.node().kind() == SyntaxKind::opt_nowait_or_skip {
            let no_wait_clause = self.visit_opt_nowait_or_skip(cursor, src)?;
            clauses.push(no_wait_clause);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::for_locking_item, src);
        Ok(())
    }

    fn visit_for_locking_strength(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // for_locking_strength
        // - FOR UPDATE
        // - FOR NO KEY UPDATE
        // - FOR SHARE
        // - FOR KEY SHARE
        cursor.goto_first_child();

        // cursor -> FOR
        pg_ensure_kind!(cursor, SyntaxKind::FOR, src);
        let mut clause = Clause::from_pg_node(cursor.node());

        while cursor.goto_next_sibling() {
            clause.pg_extend_kw(cursor.node());
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::for_locking_strength, src);

        Ok(clause)
    }

    fn visit_locked_rels_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clause: &mut Clause,
    ) -> Result<(), UroboroSQLFmtError> {
        // locked_rels_list
        // - OF qualified_name_list

        cursor.goto_first_child();

        clause.pg_extend_kw(cursor.node());
        cursor.goto_next_sibling();

        // cursor -> comments?
        self.pg_consume_comments_in_clause(cursor, clause)?;

        // cursor -> qualified_name_list
        let table_name_list = self.visit_qualified_name_list(cursor, src)?;

        clause.set_body(table_name_list);

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::locked_rels_list, src);

        Ok(())
    }

    fn visit_qualified_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // qualified_name_list
        // - qualified_name (',' qualified_name)*

        cursor.goto_first_child();

        let mut separated_lines = SeparatedLines::new();

        let first_element: Expr = self.visit_qualified_name(cursor, src)?.into();
        separated_lines.add_expr(first_element.to_aligned(), None, vec![]);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    separated_lines.add_comment_to_child(comment)?;
                }
                SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // コメントノードはリストにおける最後の要素になることはないため panic しない
                    let sibling_node = cursor.node().next_sibling().unwrap();

                    // コメントがバインドパラメータであれば次の要素を走査し、得られた式に対してバインドパラメータとして付与する
                    // そうでなければコメントをそのまま追加する
                    if sibling_node.kind() == SyntaxKind::qualified_name
                        && comment.loc().is_next_to(&sibling_node.range().into())
                    {
                        cursor.goto_next_sibling();

                        let mut element: Expr = self.visit_qualified_name(cursor, src)?.into();
                        element.set_head_comment(comment);

                        separated_lines.add_expr(
                            element.to_aligned(),
                            Some(COMMA.to_string()),
                            vec![],
                        );
                    } else {
                        separated_lines.add_comment_to_child(comment)?;
                    }
                }
                SyntaxKind::qualified_name => {
                    let element: Expr = self.visit_qualified_name(cursor, src)?.into();
                    separated_lines.add_expr(element.to_aligned(), Some(COMMA.to_string()), vec![]);
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_qualified_name_list: unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::qualified_name_list, src);

        Ok(Body::SepLines(separated_lines))
    }

    fn visit_opt_nowait_or_skip(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // opt_nowait_or_skip
        // - NOWAIT
        // - SKIP LOCKED

        cursor.goto_first_child();

        let mut clause = Clause::from_pg_node(cursor.node());

        if cursor.goto_next_sibling() {
            clause.pg_extend_kw(cursor.node());
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_nowait_or_skip, src);

        Ok(clause)
    }
}
