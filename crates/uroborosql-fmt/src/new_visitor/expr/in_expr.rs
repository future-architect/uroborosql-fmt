use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor, COMMENT},
};

impl Visitor {
    /// IN式に対して、AlignedExprを返す。
    /// IN式は、(expr NOT? IN tuple) という構造をしている。
    pub(crate) fn visit_in_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let lhs = self.visit_expr(cursor, src)?;
        cursor.goto_next_sibling();

        // NOT IN または、IN
        let mut op = String::new();
        if cursor.node().kind() == "NOT" {
            op.push_str(&convert_keyword_case(
                cursor.node().utf8_text(src.as_bytes()).unwrap(),
            ));
            op.push(' ');
            cursor.goto_next_sibling();
        }

        ensure_kind(cursor, "IN", src)?;
        op.push_str(&convert_keyword_case(
            cursor.node().utf8_text(src.as_bytes()).unwrap(),
        ));
        cursor.goto_next_sibling();

        let bind_param = if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        ensure_kind(cursor, "tuple", src)?;
        // body のネスト分と、開きかっこのネストで、二重にネストさせる。
        // TODO: body の走査に入った時点で、ネストするべきかもしれない。

        cursor.goto_first_child();
        let mut column_list = self.visit_column_list(cursor, src)?;
        cursor.goto_parent();

        ensure_kind(cursor, "tuple", src)?;

        if let Some(comment) = bind_param {
            if comment.is_block_comment() && comment.loc().is_next_to(&column_list.loc()) {
                column_list.set_head_comment(comment);
            } else {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_in_expr(): unexpected comment\n{comment:?}\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        let rhs = Expr::ColumnList(Box::new(column_list));

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(op), rhs);

        cursor.goto_parent();
        ensure_kind(cursor, "in_expression", src)?;

        Ok(aligned)
    }
}
