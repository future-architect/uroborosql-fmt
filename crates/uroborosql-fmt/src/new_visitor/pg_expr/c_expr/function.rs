use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        AlignedExpr, AsteriskExpr, Comment, Expr, FunctionCall, FunctionCallArgs, FunctionCallKind,
        PrimaryExpr, PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
    util::convert_keyword_case,
};

use super::Visitor;

impl Visitor {
    pub fn visit_func_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // func_expr
        // - func_application + within_group_clause + filter_clause + over_clause
        // - func_expr_common_subexpr
        // - json_aggregate_func + filter_clause + over_clause

        cursor.goto_first_child();
        // cursor -> func_application | func_expr_common_subexpr | json_aggregate_func

        let func = match cursor.node().kind() {
            SyntaxKind::func_application => self.visit_func_application(cursor, src)?,
            SyntaxKind::func_expr_common_subexpr => {
                self.visit_func_expr_common_subexpr(cursor, src)?
            }
            SyntaxKind::json_aggregate_func => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_expr(): json_aggregate_func is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_expr(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_next_sibling();
        // cursor ->  within_group_clause?
        if cursor.node().kind() == SyntaxKind::within_group_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): within_group_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> filter_clause?
        if cursor.node().kind() == SyntaxKind::filter_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): filter_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> over_clause?
        if cursor.node().kind() == SyntaxKind::over_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): over_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();

        pg_ensure_kind(cursor, SyntaxKind::func_expr, src)?;

        Ok(Expr::FunctionCall(Box::new(func)))
    }

    fn visit_func_expr_common_subexpr(
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

    fn visit_func_application(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // func_application
        // - func_name '(' ')'
        // - func_name '(' '*' ')'
        // - func_name '('  (ALL|DISTINCT|VARIADIC)? func_arg_list opt_sort_clause? ')'
        // - func_name '(' func_arg_list ',' VARIADIC func_arg_expr opt_sort_clause ')'

        cursor.goto_first_child();
        // cursor -> func_name
        pg_ensure_kind(cursor, SyntaxKind::func_name, src)?;
        let func_name = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;

        let args = self.handle_function_call_args(cursor, src)?;

        let func_call = FunctionCall::new(
            func_name,
            args,
            FunctionCallKind::UserDefined,
            cursor.node().range().into(),
        );

        if cursor.node().kind() == SyntaxKind::opt_sort_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_application(): opt_sort_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;
        cursor.goto_parent();

        // cursor -> func_application
        pg_ensure_kind(cursor, SyntaxKind::func_application, src)?;

        Ok(func_call)
    }

    /// 呼出時、cursor は LParen を指している
    /// 呼出後、cursor は RParen または opt_sort_clause を指している
    fn handle_function_call_args(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCallArgs, UroboroSQLFmtError> {
        // function_call_args というノードは存在しない
        // '(' function_call_args opt_sort_clause? ')'
        //  ^                      ^
        //  |                      |
        //  └ 呼出前                └ 呼出後
        //
        // function_call_args
        // - '*'
        // - (ALL|DISTINCT|VARIADIC)? func_arg_list
        // - func_arg_list ',' VARIADIC func_arg_expr

        let mut function_call_args = FunctionCallArgs::new(vec![], cursor.node().range().into());

        pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;

        cursor.goto_next_sibling();

        // 引数が空の場合
        if cursor.node().kind() == SyntaxKind::RParen {
            return Ok(function_call_args);
        }

        // ALL | DISTINCT | VARIADIC ?
        match cursor.node().kind() {
            SyntaxKind::ALL | SyntaxKind::DISTINCT | SyntaxKind::VARIADIC => {
                let all_distinct_clause = pg_create_clause(cursor, cursor.node().kind())?;
                function_call_args.set_all_distinct(all_distinct_clause);

                cursor.goto_next_sibling();
            }
            _ => {}
        }

        // cursor -> Star | func_arg_list
        match cursor.node().kind() {
            SyntaxKind::Star => {
                let asterisk_expr = AsteriskExpr::new(
                    convert_keyword_case(cursor.node().text()),
                    cursor.node().range().into(),
                );
                function_call_args.add_expr(Expr::Asterisk(Box::new(asterisk_expr)).to_aligned());
            }
            SyntaxKind::func_arg_list => {
                self._visit_func_arg_list(cursor, src, &mut function_call_args)?;
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_call_args(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        cursor.goto_next_sibling();

        // cursor -> comment?
        if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            function_call_args.set_trailing_comment(comment)?;

            cursor.goto_next_sibling();
        }

        // cursor -> Comma?
        if cursor.node().kind() == SyntaxKind::Comma {
            cursor.goto_next_sibling();
            // cursor -> VARIADIC
            if cursor.node().kind() == SyntaxKind::VARIADIC {
                // 通常の引数と可変長引数の組み合わせのパターン
                // e.g. concat_ws('a', 'b', VARIADIC array['c', 'd'])
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_call_args(): VARIADIC after func_arg_list is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        // cursor -> opt_sort_clause | ')'

        Ok(function_call_args)
    }

    /// 呼出時、cursor は func_arg_list を指している
    /// 引数に FunctionCallArgs を受け取り、ミュータブルに追加する
    fn _visit_func_arg_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        function_call_args: &mut FunctionCallArgs,
    ) -> Result<(), UroboroSQLFmtError> {
        // func_arg_list
        // - func_arg_expr (',' func_arg_expr)*

        cursor.goto_first_child();

        let first_arg = self.visit_func_arg_expr(cursor, src)?;
        function_call_args.add_expr(first_arg);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::func_arg_expr => {
                    let arg = self.visit_func_arg_expr(cursor, src)?;
                    function_call_args.add_expr(arg);
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    function_call_args.set_trailing_comment(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_func_arg_list(): unexpected node kind\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::func_arg_list, src)?;

        Ok(())
    }

    /// 呼出し後、 cursor は func_arg_expr を指している
    fn visit_func_arg_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // func_arg_expr
        // - a_expr
        // - param_name COLON_EQUALS|EQUALS_GREATER a_expr

        cursor.goto_first_child();

        let arg = match cursor.node().kind() {
            SyntaxKind::a_expr => {
                let expr = self.visit_a_expr(cursor, src)?;
                expr.to_aligned()
            }
            // 名前付き引数
            SyntaxKind::param_name => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_arg_expr(): named argument is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_arg_expr(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::func_arg_expr, src)?;

        Ok(arg)
    }
}
