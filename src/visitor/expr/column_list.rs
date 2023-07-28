use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor, COMMENT},
};

impl Visitor {
    /// カラムリストをColumnListで返す
    /// カラムリストは "(" 式 ["," 式 ...] ")"という構造になっている
    pub(crate) fn visit_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(")?;

        // ColumnListの位置
        let mut loc = Location::new(cursor.node().range());

        cursor.goto_next_sibling();

        // カラムリストが空の場合
        if cursor.node().kind() == ")" {
            return Ok(ColumnList::new(vec![], loc));
        }

        let mut exprs = vec![self.visit_expr(cursor, src)?.to_aligned()];

        // カンマ区切りの式
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                "," => {
                    cursor.goto_next_sibling();
                    exprs.push(self.visit_expr(cursor, src)?.to_aligned());
                }
                ")" => break,
                COMMENT => {
                    // 末尾コメントを想定する

                    let comment = Comment::new(cursor.node(), src);

                    // exprs は必ず1つ以上要素を持っている
                    let last = exprs.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // バインドパラメータ、末尾コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_column_list(): Unexpected comment\nnode_kind: {}\n{:#?}",
                            cursor.node().kind(),
                            cursor.node().range(),
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_column_list(): Unexpected node\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )));
                }
            }
        }

        Ok(ColumnList::new(exprs, loc))
    }
}
