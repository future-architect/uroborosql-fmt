use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor, COMMA, COMMENT},
};

impl Visitor {
    /// カラムリストをColumnListで返す
    /// カラムリストは "(" 式 ["," 式 ...] ")"という構造になっている
    pub(crate) fn visit_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(", src)?;

        // ColumnListの位置
        let mut loc = Location::new(cursor.node().range());

        cursor.goto_next_sibling();

        // カラムリストが空の場合
        if cursor.node().kind() == ")" {
            return Ok(ColumnList::new(vec![], loc));
        }

        // 開きかっこと式の間にあるコメントを保持
        // 最後の要素はバインドパラメータの可能性があるので、最初の式を処理した後で付け替える
        let mut comment_buf = vec![];
        while cursor.node().kind() == COMMENT {
            comment_buf.push(Comment::new(cursor.node(), src));
            cursor.goto_next_sibling();
        }

        let mut fist_expr = self.visit_expr(cursor, src)?;

        // ```
        // (
        // -- comment
        //     /* bind */expr ...
        //     ^^^^^^^^^^ comment_buf.last()
        //```
        // 開き括弧の後のコメントのうち最後のもの（最初の式の直前にあるもの）を取得
        if let Some(comment) = comment_buf.last() {
            if comment.is_block_comment() && comment.loc().is_next_to(&fist_expr.loc()) {
                // ブロックコメントかつ式に隣接していればバインドパラメータなので、式に付与する
                fist_expr.set_head_comment(comment.clone());
                // comment_buf からも削除
                comment_buf.pop().unwrap();
            }
        }

        let mut exprs = vec![fist_expr.to_aligned()];

        // カンマ区切りの式
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                COMMA => {
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
                            "visit_column_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_column_list(): Unexpected node\nnode_kind: {}\n{}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        let mut column_list = ColumnList::new(exprs, loc);

        // 開き括弧の後のコメントを追加
        for comment in comment_buf {
            column_list.add_start_comment(comment)
        }

        Ok(column_list)
    }
}
