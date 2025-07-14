use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        AlignedExpr, AsteriskExpr, Comment, Expr, FunctionCall, FunctionCallArgs, FunctionCallKind,
        Location,
    },
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor},
};

use super::Visitor;

impl Visitor {
    pub fn visit_func_application(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // func_application
        // - func_name '(' ')'
        // - func_name '(' '*' ')'
        // - func_name '('  (ALL|DISTINCT|VARIADIC)? func_arg_list sort_clause? ')'
        // - func_name '(' func_arg_list ',' VARIADIC func_arg_expr sort_clause? ')'

        let parent_loc = cursor.node().range();

        cursor.goto_first_child();
        // cursor -> func_name
        ensure_kind!(cursor, SyntaxKind::func_name, src);
        let func_name = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);

        let mut args = self.handle_function_call_args(cursor, src)?;

        // cursor -> sort_clause | ')'
        if cursor.node().kind() == SyntaxKind::sort_clause {
            let sort_clause = self.visit_sort_clause(cursor, src)?;
            args.set_order_by(sort_clause);
            cursor.goto_next_sibling();
        }

        let func_call = FunctionCall::new(
            func_name,
            args,
            FunctionCallKind::UserDefined,
            parent_loc.into(),
        );

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        cursor.goto_parent();

        // cursor -> func_application
        ensure_kind!(cursor, SyntaxKind::func_application, src);

        Ok(func_call)
    }

    /// 呼出時、cursor は LParen を指している
    /// 呼出後、cursor は RParen または sort_clause を指している
    fn handle_function_call_args(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCallArgs, UroboroSQLFmtError> {
        // function_call_args というノードは存在しない
        // '(' function_call_args sort_clause? ')'
        //  ^                      ^
        //  |                      |
        //  └ 呼出前                └ 呼出後
        //
        // function_call_args
        // - '*'
        // - (ALL|DISTINCT|VARIADIC)? func_arg_list
        // - func_arg_list ',' VARIADIC func_arg_expr

        let mut function_call_args = FunctionCallArgs::new(vec![], cursor.node().range().into());

        ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();

        // 引数が空の場合
        if cursor.node().kind() == SyntaxKind::RParen {
            return Ok(function_call_args);
        }

        // ALL | DISTINCT | VARIADIC ?
        match cursor.node().kind() {
            SyntaxKind::ALL | SyntaxKind::DISTINCT | SyntaxKind::VARIADIC => {
                let all_distinct_clause = create_clause!(cursor, expr: cursor.node().kind());
                function_call_args.set_all_distinct(all_distinct_clause);

                cursor.goto_next_sibling();
            }
            _ => {}
        }

        // cursor -> bind param?
        let first_arg_bind_param = if cursor.node().kind() == SyntaxKind::C_COMMENT {
            let comment = Comment::new(cursor.node());
            cursor.goto_next_sibling();

            Some(comment)
        } else {
            None
        };

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
                self.visit_func_arg_list(
                    cursor,
                    src,
                    &mut function_call_args,
                    first_arg_bind_param,
                )?;
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_function_call_args(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        cursor.goto_next_sibling();

        // cursor -> comment?
        if cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
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
                    "handle_function_call_args(): VARIADIC after func_arg_list is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        // cursor -> sort_clause | ')'

        Ok(function_call_args)
    }

    /// 呼出時、cursor は func_arg_list を指している
    /// 引数に FunctionCallArgs を受け取り、ミュータブルに追加する
    /// first_arg_bind_param には、最初の引数のバインドパラメータになりうるコメントを渡す
    pub(crate) fn visit_func_arg_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        function_call_args: &mut FunctionCallArgs,
        first_arg_bind_param: Option<Comment>,
    ) -> Result<(), UroboroSQLFmtError> {
        // func_arg_list
        // - func_arg_expr (',' func_arg_expr)*

        cursor.goto_first_child();

        let mut first_arg = self.visit_func_arg_expr(cursor, src)?;

        // 直前のコメントが最初の引数のバインドパラメータであれば追加する
        if let Some(bind_param) = first_arg_bind_param {
            if bind_param.is_block_comment() && bind_param.loc().is_next_to(&first_arg.loc()) {
                first_arg.set_head_comment(bind_param);
            } else {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_arg_list(): Comments are not supported at this position\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        function_call_args.add_expr(first_arg);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::func_arg_expr => {
                    let arg = self.visit_func_arg_expr(cursor, src)?;
                    function_call_args.add_expr(arg);
                }
                SyntaxKind::C_COMMENT => {
                    // バインドパラメータを想定する
                    let comment = Comment::new(cursor.node());

                    cursor.goto_next_sibling();
                    ensure_kind!(cursor, SyntaxKind::func_arg_expr, src);
                    if comment
                        .loc()
                        .is_next_to(&Location::from(cursor.node().range()))
                    {
                        let mut arg = self.visit_func_arg_expr(cursor, src)?;
                        arg.set_head_comment(comment);

                        function_call_args.add_expr(arg);
                    } else {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_func_arg_list(): Comments are not supported at this position\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());
                    function_call_args.set_trailing_comment(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_func_arg_list(): unexpected node kind\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_arg_list, src);

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
                let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
                expr.to_aligned()
            }
            // 名前付き引数
            SyntaxKind::param_name => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_arg_expr(): named argument is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_arg_expr(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_arg_expr, src);

        Ok(arg)
    }
}
