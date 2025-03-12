use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        AlignedExpr, Expr, FunctionCall, FunctionCallArgs, FunctionCallKind, PrimaryExpr,
        PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
    util::convert_keyword_case,
};

use super::Visitor;
impl Visitor {
    pub fn visit_func_expr_common_subexpr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // func_expr_common_subexpr
        // - COLLATION FOR '(' a_expr ')'
        // - CURRENT_DATE
        // - CURRENT_TIME
        // - CURRENT_TIME '(' a_expr ')'
        // - CURRENT_TIMESTAMP
        // - CURRENT_TIMESTAMP '(' a_expr ')'
        // - LOCALTIME
        // - LOCALTIME '(' a_expr ')'
        // - LOCALTIMESTAMP
        // - LOCALTIMESTAMP '(' a_expr ')'
        // - CURRENT_ROLE
        // - CURRENT_USER
        // - SESSION_USER
        // - USER
        // - CURRENT_CATALOG
        // - CURRENT_SCHEMA
        // - CAST '(' a_expr AS typename ')'
        // - EXTRACT '(' extract_list ')'
        // - NORMALIZE '(' a_expr ')'
        // - NORMALIZE '(' a_expr ',' unicode_normal_form ')'
        // - OVERLAY '(' overlay_list ')'
        // - POSITION '(' position_list ')'
        // - SUBSTRING '(' substr_list ')'
        // - TREAT '(' a_expr AS typename ')'
        // - TRIM '(' BOTH trim_list ')'
        // - TRIM '(' LEADING trim_list ')'
        // - TRIM '(' TRAILING trim_list ')'
        // - TRIM '(' trim_list ')'
        // - NULLIF '(' a_expr ',' a_expr ')'
        // - COALESCE '(' expr_list ')'
        // - GREATEST '(' expr_list ')'
        // - LEAST '(' expr_list ')'
        // - XMLCONCAT '(' expr_list ')'
        // - XMLELEMENT '(' NAME_P ColLabel ')'
        // - XMLELEMENT '(' NAME_P ColLabel ',' xml_attributes ')'
        // - XMLELEMENT '(' NAME_P ColLabel ',' expr_list ')'
        // - XMLELEMENT '(' NAME_P ColLabel ',' xml_attributes ',' expr_list ')'
        // - XMLEXISTS '(' c_expr xmlexists_argument ')'
        // - XMLFOREST '(' xml_attribute_list ')'
        // - XMLPARSE '(' document_or_content a_expr xml_whitespace_option ')'
        // - XMLPI '(' NAME_P ColLabel ')'
        // - XMLPI '(' NAME_P ColLabel ',' a_expr ')'
        // - XMLROOT '(' a_expr ',' xml_root_version opt_xml_root_standalone ')'
        // - XMLSERIALIZE '(' document_or_content a_expr AS SimpleTypename ')'
        // - special_function

        cursor.goto_first_child();

        let func = match cursor.node().kind() {
            SyntaxKind::CAST => self.handle_cast_function(cursor, src)?,
            _ => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_expr_common_subexpr(): function `{}` is not implemented\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::func_expr_common_subexpr, src)?;

        Ok(func)
    }

    /// 呼出時、cursor は CAST キーワード を指している
    /// 呼出後、cursor は 最後の要素の RParen を指している
    fn handle_cast_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // CAST '(' a_expr AS typename ')'

        // cursor -> CAST
        pg_ensure_kind(cursor, SyntaxKind::CAST, src)?;
        let cast_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;

        cursor.goto_next_sibling();
        // cursor -> a_expr
        pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;
        let expr = self.visit_a_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> AS
        pg_ensure_kind(cursor, SyntaxKind::AS, src)?;
        let as_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> Typename
        pg_ensure_kind(cursor, SyntaxKind::Typename, src)?;
        let type_name = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

        // 最後の要素
        assert!(!cursor.goto_next_sibling());

        let mut aligned = AlignedExpr::new(expr);
        aligned.add_rhs(Some(as_keyword), Expr::Primary(Box::new(type_name)));

        let args = FunctionCallArgs::new(vec![aligned], cursor.node().range().into());

        let function = FunctionCall::new(
            cast_keyword,
            args,
            FunctionCallKind::BuiltIn,
            cursor.node().range().into(),
        );

        Ok(function)
    }
}
