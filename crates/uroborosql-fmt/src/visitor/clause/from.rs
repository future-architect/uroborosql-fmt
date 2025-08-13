mod table_ref;

use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{from_list::FromList, *},
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMA},
};

impl Visitor {
    pub(crate) fn visit_from_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clause = "FROM" from_list

        // cursor -> "FROM"
        cursor.goto_first_child();
        ensure_kind!(cursor, SyntaxKind::FROM, src);

        let mut clause = create_clause!(cursor, SyntaxKind::FROM);
        cursor.goto_next_sibling();

        self.consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> Comma?
        let extra_leading_comma = if cursor.node().kind() == SyntaxKind::Comma {
            cursor.goto_next_sibling();
            Some(COMMA.to_string())
        } else {
            None
        };

        self.consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> from_list
        ensure_kind!(cursor, SyntaxKind::from_list, src);

        let from_list = self.visit_from_list(cursor, src, extra_leading_comma)?;

        clause.set_body(from_list);

        // cursor -> from_clause
        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::from_clause, src);

        Ok(clause)
    }

    /// 呼出し後、cursor は from_list を指している
    /// 直前にカンマがある場合は extra_leading_comma として渡す
    pub(crate) fn visit_from_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
        extra_leading_comma: Option<String>,
    ) -> Result<Body, UroboroSQLFmtError> {
        // from_list -> table_ref ("," table_ref)*

        // from_listは必ず table_ref を子供に持つ
        // cursor -> table_ref
        cursor.goto_first_child();
        ensure_kind!(cursor, SyntaxKind::table_ref, src);

        let mut from_body = FromList::new();
        from_body.set_extra_leading_comma(extra_leading_comma);

        let table_ref = self.visit_table_ref(cursor, src)?;
        from_body.add_table_ref(table_ref);

        while cursor.goto_next_sibling() {
            // cursor -> "," または table_ref
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::table_ref => {
                    let table_ref = self.visit_table_ref(cursor, src)?;
                    from_body.add_table_ref(table_ref);
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());
                    from_body.add_comment_to_child(comment)?;
                }
                SyntaxKind::C_COMMENT => {
                    let comment_node = cursor.node();
                    let comment = Comment::new(comment_node);

                    let Some(next_sibling) = cursor.node().next_sibling() else {
                        // 最後の要素の行末にあるコメントは、 from_list の直下に現れず from_list と同階層の要素になる
                        // そのためコメントが最後の子供になることはなく、次のノードを必ず取得できる
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_from_list(): unexpected node kind\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    };

                    // テーブル参照における置換文字列
                    if comment.loc().is_next_to(&next_sibling.range().into()) {
                        cursor.goto_next_sibling();
                        // cursor -> table_ref
                        ensure_kind!(cursor, SyntaxKind::table_ref, src);
                        let mut table_ref = self.visit_table_ref(cursor, src)?;

                        // 置換文字列をセット
                        table_ref.set_head_comment(comment);
                        from_body.add_table_ref(table_ref);
                    } else {
                        from_body.add_comment_to_child(comment)?;
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_from_list(): unexpected node kind: {}\n{}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // cursor -> from_list
        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::from_list, src);

        Ok(Body::FromList(from_body))
    }
}
