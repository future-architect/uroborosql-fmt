mod expr_as_typename_function;
mod expr_list_function;
mod nullif_function;
mod only_keyword_function;
mod optional_iconst_function;
mod substring_function;
mod trim_function;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::Expr,
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
};

use super::Visitor;

impl Visitor {
    pub fn visit_func_expr_common_subexpr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // func_expr_common_subexpr:
        //
        // only keyword
        // - CURRENT_DATE
        // - CURRENT_ROLE
        // - CURRENT_USER
        // - SESSION_USER
        // - SYSTEM_USER
        // - USER
        // - CURRENT_CATALOG
        // - CURRENT_SCHEMA
        //
        // keyword '(' expr_list ')'
        // - COALESCE '(' expr_list ')'
        // - GREATEST '(' expr_list ')'
        // - LEAST '(' expr_list ')'
        // - XMLCONCAT '(' expr_list ')'
        //
        // keyword '(' a_expr ')'
        // - JSON_SCALAR '(' a_expr ')'
        // - COLLATION FOR '(' a_expr ')'
        //
        // keyword '(' a_expr AS Typename ')'
        // - CAST '(' a_expr AS Typename ')'
        // - TREAT '(' a_expr AS Typename ')'
        //
        // only keyword or keyword '(' Iconst ')'
        // - CURRENT_TIME
        // - CURRENT_TIME '(' Iconst ')'
        // - CURRENT_TIMESTAMP
        // - CURRENT_TIMESTAMP '(' Iconst ')'
        // - LOCALTIME
        // - LOCALTIME '(' Iconst ')'
        // - LOCALTIMESTAMP
        // - LOCALTIMESTAMP '(' Iconst ')'
        //
        // keyword '(' NAME_P ColLabel ')'
        // - XMLPI '(' NAME_P ColLabel ')'
        // - XMLELEMENT '(' NAME_P ColLabel ')'
        //
        // ---------------------------------------
        //
        // - TRIM '(' trim_list ')'
        // - TRIM '(' BOTH trim_list ')'
        // - TRIM '(' LEADING trim_list ')'
        // - TRIM '(' TRAILING trim_list ')'
        // - MERGE_ACTION '(' ')'
        // - NULLIF '(' a_expr ',' a_expr ')'
        // - EXTRACT '(' extract_list ')'
        // - POSITION '(' position_list ')'
        // - XMLFOREST '(' xml_attribute_list ')'
        // - SUBSTRING '(' substr_list ')'
        // - SUBSTRING '(' func_arg_list_opt ')'
        // - NORMALIZE '(' a_expr ')'
        // - NORMALIZE '(' a_expr ',' unicode_normal_form ')'
        // - OVERLAY '(' overlay_list ')'
        // - OVERLAY '(' func_arg_list_opt ')'
        // - JSON_OBJECT '(' func_arg_list ')'
        // - JSON_OBJECT '(' json_name_and_value_list json_object_constructor_null_clause_opt json_key_uniqueness_constraint_opt json_returning_clause_opt ')'
        // - JSON_OBJECT '(' json_returning_clause_opt ')'
        // - JSON_ARRAY '(' json_value_expr_list json_array_constructor_null_clause_opt json_returning_clause_opt ')'
        // - JSON_ARRAY '(' select_no_parens json_format_clause_opt json_returning_clause_opt ')'
        // - JSON_ARRAY '(' json_returning_clause_opt ')'
        // - XMLEXISTS '(' c_expr xmlexists_argument ')'
        // - XMLPI '(' NAME_P ColLabel ',' a_expr ')'
        // - XMLELEMENT '(' NAME_P ColLabel ',' xml_attributes ')'
        // - XMLELEMENT '(' NAME_P ColLabel ',' expr_list ')'
        // - XMLELEMENT '(' NAME_P ColLabel ',' xml_attributes ',' expr_list ')'
        // - XMLPARSE '(' document_or_content a_expr xml_whitespace_option ')'
        // - XMLSERIALIZE '(' document_or_content a_expr AS SimpleTypename xml_indent_option ')'
        // - XMLROOT '(' a_expr ',' xml_root_version opt_xml_root_standalone ')'
        // - JSON '(' json_value_expr json_key_uniqueness_constraint_opt ')'
        // - JSON_SERIALIZE '(' json_value_expr json_returning_clause_opt ')'
        // - JSON_QUERY '(' json_value_expr ',' a_expr json_passing_clause_opt json_returning_clause_opt json_wrapper_behavior json_quotes_clause_opt json_behavior_clause_opt ')'
        // - JSON_EXISTS '(' json_value_expr ',' a_expr json_passing_clause_opt json_on_error_clause_opt ')'
        // - JSON_VALUE '(' json_value_expr ',' a_expr json_passing_clause_opt json_returning_clause_opt json_behavior_clause_opt ')'
        //

        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            // only keyword
            kind @ (SyntaxKind::CURRENT_DATE
            | SyntaxKind::CURRENT_ROLE
            | SyntaxKind::CURRENT_USER
            | SyntaxKind::SESSION_USER
            | SyntaxKind::SYSTEM_USER
            | SyntaxKind::USER
            | SyntaxKind::CURRENT_CATALOG
            | SyntaxKind::CURRENT_SCHEMA) => {
                let keyword = self.handle_only_keyword_function(cursor, src, kind)?;
                // キーワードのみの関数は PrimaryExpr として扱う
                Expr::Primary(Box::new(keyword))
            }

            // keyword '(' expr_list ')'
            kind @ (SyntaxKind::COALESCE
            | SyntaxKind::GREATEST
            | SyntaxKind::LEAST
            | SyntaxKind::XMLCONCAT) => {
                let func_call = self.handle_expr_list_function(cursor, src, kind)?;
                Expr::FunctionCall(Box::new(func_call))
            }

            // keyword '(' a_expr AS Typename ')'
            SyntaxKind::CAST => {
                // TREAT function has the same structure as CAST, but it is not supported for now
                let func_call =
                    self.handle_expr_as_typename_function(cursor, src, SyntaxKind::CAST)?;
                Expr::FunctionCall(Box::new(func_call))
            }

            // only keyword or keyword '(' Iconst ')'
            kind @ (SyntaxKind::CURRENT_TIME
            | SyntaxKind::CURRENT_TIMESTAMP
            | SyntaxKind::LOCALTIME
            | SyntaxKind::LOCALTIMESTAMP) => {
                self.handle_optional_iconst_function(cursor, src, kind)?
            }

            SyntaxKind::TRIM => {
                // TRIM '(' trim_list ')'
                // TRIM '(' BOTH trim_list ')'
                // TRIM '(' LEADING trim_list ')'
                // TRIM '(' TRAILING trim_list ')'
                let func_call = self.handle_trim_function(cursor, src)?;
                Expr::FunctionCall(Box::new(func_call))
            }
            SyntaxKind::SUBSTRING => {
                // SUBSTRING '(' substr_list ')'
                // SUBSTRING '(' func_arg_list_opt ')'
                let func_call = self.handle_substring_function(cursor, src)?;
                Expr::FunctionCall(Box::new(func_call))
            }
            SyntaxKind::NULLIF => {
                // nullif '(' a_expr ',' a_expr ')'
                let func_call = self.handle_nullif_function(cursor, src)?;
                Expr::FunctionCall(Box::new(func_call))
            }
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

        Ok(expr)
    }
}
