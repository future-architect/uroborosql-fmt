use crate::{
    config::CONFIG,
    cst::{type_cast::TypeCast, *},
    error::UroboroSQLFmtError,
    new_visitor::{ensure_kind, Visitor},
    util::convert_keyword_case,
};

use tree_sitter::TreeCursor;

impl Visitor {
    /// 設定ファイルの`convert_double_colon_cast`がtrueで、かつ`X::type`でキャストされている場合`CAST(X AS type)`に変換を行う
    pub(crate) fn visit_type_cast(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        let cast_loc = Location::new(cursor.node().range());

        cursor.goto_first_child();

        if cursor.node().kind() == "CAST" {
            // CAST関数
            let cast_keyword =
                convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());

            cursor.goto_next_sibling();
            ensure_kind(cursor, "(", src)?;
            cursor.goto_next_sibling();

            // キャストされる式
            // 注: キャスト関数の式は alias ノードになっていないので、
            // visit_aliasable_expr では対処できない。
            let expr = self.visit_expr(cursor, src)?;
            cursor.goto_next_sibling();
            ensure_kind(cursor, "AS", src)?;
            let as_keyword = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());

            cursor.goto_next_sibling();

            ensure_kind(cursor, "type", src)?;
            // 型は特殊な書き方をされていないことを想定し、ソースの文字列をそのまま PrimaryExpr に変換する。
            // 例えば、"CHAR   ( 3    )" などのように、途中に空白を含むような特殊な書き方をした場合、フォーマット結果にもその空白が現れてしまう。
            let type_name = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
            cursor.goto_next_sibling();

            ensure_kind(cursor, ")", src)?;

            // expr AS type を AlignedExpr にする。
            let mut aligned = AlignedExpr::new(expr);
            aligned.add_rhs(Some(as_keyword), Expr::Primary(Box::new(type_name)));
            let loc = aligned.loc();

            let args = FunctionCallArgs::new(vec![aligned], loc);

            let function =
                FunctionCall::new(cast_keyword, args, FunctionCallKind::BuiltIn, cast_loc);

            cursor.goto_parent();
            ensure_kind(cursor, "type_cast", src)?;

            Ok(Expr::FunctionCall(Box::new(function)))
        } else {
            // X::type

            let expr = self.visit_expr(cursor, src)?;

            cursor.goto_next_sibling();

            ensure_kind(cursor, "::", src)?;

            cursor.goto_next_sibling();

            let type_name = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
            ensure_kind(cursor, "type", src)?;

            cursor.goto_parent();
            ensure_kind(cursor, "type_cast", src)?;

            if CONFIG.read().unwrap().convert_double_colon_cast {
                // CAST関数に変換

                let cast_keyword = convert_keyword_case("CAST");

                let as_keyword = convert_keyword_case("AS");
                let mut aligned = expr.to_aligned();
                aligned.add_rhs(Some(as_keyword), Expr::Primary(Box::new(type_name)));

                let function = FunctionCall::new(
                    cast_keyword,
                    FunctionCallArgs::new(vec![aligned], expr.loc()),
                    FunctionCallKind::BuiltIn,
                    cast_loc,
                );

                Ok(Expr::FunctionCall(Box::new(function)))
            } else {
                let type_cast = TypeCast::new(expr, type_name, cast_loc);
                Ok(Expr::TypeCast(Box::new(type_cast)))
            }
        }
    }
}
