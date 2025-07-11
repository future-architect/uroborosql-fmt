use crate::{
    cst::{
        type_cast::TypeCast, Expr, FunctionCall, FunctionCallArgs, FunctionCallKind, PrimaryExpr,
        PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::ensure_kind,
    CONFIG,
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use super::Visitor;

impl Visitor {
    /// 式を受け取り、型変換のノード群を走査する
    ///
    /// 呼出時、 cursor は TYPECAST を指している
    /// 呼出後、 cursor は Typename を指している
    ///
    pub fn handle_typecast_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        expr: Expr,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // a_expr TYPECAST Typename
        // ^      ^        ^
        // expr   │        │
        //        └ 呼出時  └ 呼出後

        // cursor -> TYPECAST (`::`)
        ensure_kind!(cursor, SyntaxKind::TYPECAST, src);

        cursor.goto_next_sibling();
        // cursor -> Typename
        let type_name = self.visit_typename(cursor, src)?;
        ensure_kind!(cursor, SyntaxKind::Typename, src);

        // 親の a_expr が、 Type cast 全体の式にあたる
        let loc = cursor.node().parent().unwrap().range().into();

        if CONFIG.read().unwrap().convert_double_colon_cast {
            // CAST関数に変換

            let cast_keyword = convert_keyword_case("CAST");

            let as_keyword = CONFIG.read().unwrap().keyword_case.format("AS");
            let mut aligned = expr.to_aligned();
            aligned.add_rhs(Some(as_keyword), Expr::Primary(Box::new(type_name)));

            let function = FunctionCall::new(
                cast_keyword,
                FunctionCallArgs::new(vec![aligned], expr.loc()),
                FunctionCallKind::BuiltIn,
                loc,
            );

            Ok(Expr::FunctionCall(Box::new(function)))
        } else {
            let type_cast = TypeCast::new(expr, type_name, loc);
            Ok(Expr::TypeCast(Box::new(type_cast)))
        }
    }

    fn visit_typename(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<PrimaryExpr, UroboroSQLFmtError> {
        // Typename
        // - SETOF? SimpleTypename opt_array_bounds
        // - SETOF? SimpleTypename ARRAY ('[' Iconst ']')?

        ensure_kind!(cursor, SyntaxKind::Typename, src);
        // とりあえずはシンプルなキーワードのみの型名を想定
        let typename = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Keyword)?;
        Ok(typename)
    }
}
