mod a_expr;
mod c_expr;
mod expr_list;

use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{Comment, Expr},
    error::UroboroSQLFmtError,
    visitor::pg_ensure_kind,
};

use super::{pg_error_annotation_from_cursor, Visitor};

impl Visitor {
    /// a_expr または b_expr を走査する
    /// 引数で a_expr か b_expr のどちらを走査するかを指定する
    /// 呼出し時の cursor がコメントを指している場合、バインドパラメータとして隣の兄弟ノードに付加する
    /// 呼出し後、cursor は呼出し時の位置に戻る
    pub(crate) fn visit_a_expr_or_b_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // b_expr は a_expr のサブセットであるため、a_expr および b_expr の走査には a_expr 用の visitor をそのまま使う

        // 式の直前にあるコメントを処理する
        // この位置のコメントはバインドパラメータを想定するため、ブロックコメント（C_COMMENT）のみを処理する
        let head_comment_node = if cursor.node().kind() == SyntaxKind::C_COMMENT {
            let comment = cursor.node();
            cursor.goto_next_sibling();
            // 式の直前に複数コメントが来る場合は想定していない
            Some(comment)
        } else {
            None
        };

        let expr_kind = cursor.node().kind();
        cursor.goto_first_child();

        // cursor -> c_expr | DEFAULT | Plus | Minus | NOT | qual_Op | a_expr | UNIQUE
        let mut expr = self.handle_a_expr_or_b_expr_inner(cursor, src)?;

        // バインドパラメータの追加
        if let Some(comment_node) = head_comment_node {
            let comment = Comment::pg_new(comment_node.clone());
            if comment.loc().is_next_to(&expr.loc()) {
                // 式に隣接していればバインドパラメータ
                expr.set_head_comment(comment);
            } else {
                // TODO: 隣接していないコメント
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr_or_b_expr(): (bind parameter) separated comment\n{}",
                    pg_error_annotation_from_cursor(&comment_node.walk(), src)
                )));
            }
        }

        // cursor -> (last_node)
        assert!(
            !cursor.goto_next_sibling(),
            "visit_a_expr_or_b_expr(): cursor is not at the last node."
        );

        cursor.goto_parent();
        // cursor -> a_expr or b_expr (parent)
        pg_ensure_kind!(cursor, expr: expr_kind, src);

        Ok(expr)
    }

    pub fn visit_relation_expr(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // relation_expr
        // - qualified_name
        // - extended_relation_expr

        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::qualified_name => self.visit_qualified_name(cursor, src)?,
            SyntaxKind::extended_relation_expr => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_relation_expr(): extended_relation_expr node appeared. Extended relation expressions are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_relation_expr(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::relation_expr, src);

        Ok(expr.into())
    }
}
