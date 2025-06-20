mod expr_as_typename_function;
mod expr_list_function;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::FunctionCall,
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
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
            // keyword '(' a_expr AS Typename ')'
            SyntaxKind::CAST => {
                // TREAT function has the same structure as CAST, but it is not supported for now
                self.handle_expr_as_typename_function(cursor, src, SyntaxKind::CAST)?
            }
            kind @ (SyntaxKind::COALESCE
            | SyntaxKind::GREATEST
            | SyntaxKind::LEAST
            | SyntaxKind::XMLCONCAT) => self.handle_expr_list_function(cursor, src, kind)?,
            _ => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_expr_common_subexpr(): function `{}` is not implemented\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::func_expr_common_subexpr, src);

        Ok(func)
    }
}
