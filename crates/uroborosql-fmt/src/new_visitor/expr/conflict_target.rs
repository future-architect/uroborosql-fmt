use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    util::{convert_identifier_case, convert_keyword_case},
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor, COMMA},
};

impl Visitor {
    /// conflict_targetをフォーマットする
    pub(crate) fn visit_conflict_target(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTarget, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // conflict_target =
        //      ( index_column_name  [ COLLATE collation ] [ op_class ] [, ...] ) [ WHERE index_predicate ]
        //      ON CONSTRAINT constraint_name

        if cursor.node().kind() == "ON_CONSTRAINT" {
            //      ON CONSTRAINT constraint_name

            let on_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

            cursor.goto_next_sibling();
            // cursor -> "ON_CONSTRAINT"

            ensure_kind(cursor, "ON_CONSTRAINT", src)?;
            let constraint_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

            cursor.goto_next_sibling();
            // cursor -> constraint_name

            ensure_kind(cursor, "identifier", src)?;

            let constraint_name = cursor.node().utf8_text(src.as_bytes()).unwrap();

            cursor.goto_parent();
            ensure_kind(cursor, "conflict_target", src)?;

            Ok(ConflictTarget::OnConstraint(OnConstraint::new(
                (
                    convert_keyword_case(on_keyword),
                    convert_keyword_case(constraint_keyword),
                ),
                constraint_name.to_string(),
            )))
        } else {
            //      ( index_column_name  [ COLLATE collation ] [ op_class ] [, ...] ) [ WHERE index_predicate ]
            let index_column_name = self.visit_conflict_target_column_list(cursor, src)?;
            let mut specify_index_column = SpecifyIndexColumn::new(index_column_name);

            cursor.goto_next_sibling();

            // where句がある場合
            if cursor.node().kind() == "where_clause" {
                let where_clause = self.visit_where_clause(cursor, src)?;
                specify_index_column.set_where_clause(where_clause);
            }
            cursor.goto_parent();
            ensure_kind(cursor, "conflict_target", src)?;

            Ok(ConflictTarget::SpecifyIndexColumn(specify_index_column))
        }
    }

    /// conflict_targetにおけるカラムリストをフォーマットする
    /// "(" カラム名 [COLLATE collation] [op_class] [, ...] ")" という構造になっている
    pub(crate) fn visit_conflict_target_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTargetColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(", src)?;

        // ConflictTargetColumnListの位置
        let mut loc = Location::new(cursor.node().range());
        // ConflictTargetColumnListの要素
        let mut elements = vec![];

        // カラム名 [COLLATE collation] [op_class] [, ...]
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                "identifier" => {
                    let column =
                        convert_identifier_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
                    let element = ConflictTargetElement::new(column);
                    elements.push(element);
                }
                COMMA => {
                    continue;
                }
                "COLLATE" => {
                    let collate_keyword =
                        convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
                    cursor.goto_next_sibling();
                    ensure_kind(cursor, "collation", src)?;
                    cursor.goto_first_child();
                    ensure_kind(cursor, "identifier", src)?;

                    // collationはユーザが定義することも可能であるため、識別子ルールを適用
                    let collation =
                        convert_identifier_case(cursor.node().utf8_text(src.as_bytes()).unwrap());

                    // elementsの最後の要素にCOLLATEをセット
                    elements
                        .last_mut()
                        .unwrap()
                        .set_collate(Collate::new(collate_keyword, collation));
                    cursor.goto_parent();
                }
                "op_class" => {
                    cursor.goto_first_child();
                    ensure_kind(cursor, "identifier", src)?;
                    let op_class =
                        convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());

                    // elementsの最後の要素にop_classをセット
                    elements.last_mut().unwrap().set_op_class(op_class);
                    cursor.goto_parent();
                }
                ")" => break,
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_conflict_target_column_list(): Unexpected node\nnode_kind: {}\n{}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }
        Ok(ConflictTargetColumnList::new(elements, loc))
    }
}
