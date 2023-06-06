mod binary;
mod boolean;
mod cond;
mod function;
mod subquery;

use tree_sitter::TreeCursor;

use crate::{cst::*, util::convert_keyword_case};

use super::{ensure_kind, Formatter, COMMENT};

impl Formatter {
    /// エイリアス可能な式
    /// 呼び出し後、cursorはaliasまたは式のノードを指している
    pub(crate) fn format_aliasable_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // エイリアス可能な式の定義
        //    _aliasable_expression =
        //        alias | _expression

        //    alias =
        //        _expression
        //        ["AS"]
        //        identifier

        let comment = if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        match cursor.node().kind() {
            "alias" => {
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let mut lhs_expr = self.format_expr(cursor, src)?;
                if let Some(comment) = comment {
                    if comment.loc().is_next_to(&lhs_expr.loc()) {
                        lhs_expr.set_head_comment(comment);
                    } else {
                        // エイリアス式の直前のコメントは、バインドパラメータしか考慮していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                            "format_aliasable_expr(): unexpected comment\n{:?}",
                            cursor.node().range()
                        )));
                    }
                }

                let mut aligned = AlignedExpr::new(lhs_expr, true);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() {
                    // cursor -> trailing_comment | "AS"?

                    if cursor.node().kind() == COMMENT {
                        // ASの直前にcommentがある場合
                        let comment = Comment::new(cursor.node(), src);

                        if comment.is_multi_line_comment()
                            || !comment.loc().is_same_line(&aligned.loc())
                        {
                            // 行末以外のコメント(次以降の行のコメント)は未定義
                            // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                            // エイリアスがない場合は、コメントノードがここに現れない
                            return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                                "format_aliasable_expr(): unexpected syntax\nnode_kind: {}\n{:#?}",
                                cursor.node().kind(),
                                cursor.node().range(),
                            )));
                        } else {
                            // 行末コメント
                            aligned.set_lhs_trailing_comment(comment)?;
                        }
                        cursor.goto_next_sibling();
                    }

                    // ASが存在する場合は読み飛ばす
                    if cursor.node().kind() == "AS" {
                        cursor.goto_next_sibling();
                    }

                    //右辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    ensure_kind(cursor, "identifier")?;

                    let rhs_expr =
                        PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Expr);
                    aligned.add_rhs(
                        convert_keyword_case("AS"),
                        Expr::Primary(Box::new(rhs_expr)),
                    );
                }

                // cursorをalias に戻す
                cursor.goto_parent();

                Ok(aligned)
            }
            _ => {
                // _expression
                let expr = self.format_expr(cursor, src)?;

                Ok(AlignedExpr::new(expr, true))
            }
        }
    }

    /// 式のフォーマットを行う。
    /// cursorがコメントを指している場合、バインドパラメータであれば結合して返す。
    /// 式の初めにバインドパラメータが現れた場合、式の本体は隣の兄弟ノードになる。
    /// 呼び出し後、cursorは式の本体のノードを指す
    pub(crate) fn format_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // バインドパラメータをチェック
        let head_comment = if cursor.node().kind() == COMMENT {
            let comment_node = cursor.node();
            cursor.goto_next_sibling();
            // cursor -> _expression
            // 式の直前に複数コメントが来る場合は想定していない
            Some(Comment::new(comment_node, src))
        } else {
            None
        };

        let mut result = match cursor.node().kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                // cursor -> dotted_name

                let range = cursor.node().range();

                cursor.goto_first_child();
                // cursor -> identifier

                let mut dotted_name = String::new();

                let id_node = cursor.node();
                dotted_name.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                while cursor.goto_next_sibling() {
                    // cursor -> . または cursor -> identifier
                    match cursor.node().kind() {
                        "." => dotted_name.push('.'),
                        "ERROR" => {
                            return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                                "format_expr: ERROR node appeared \n{:?}",
                                cursor.node().range()
                            )));
                        }
                        _ => dotted_name.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary =
                    PrimaryExpr::new(dotted_name, Location::new(range), PrimaryExprKind::Expr);

                // cursorをdotted_nameに戻す
                cursor.goto_parent();
                ensure_kind(cursor, "dotted_name")?;

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => self.format_binary_expr(cursor, src)?,
            "between_and_expression" => {
                Expr::Aligned(Box::new(self.format_between_and_expression(cursor, src)?))
            }
            "boolean_expression" => self.format_bool_expr(cursor, src)?,
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                // defaultの場合はキーワードとして扱う
                let primary = if "default"
                    .eq_ignore_ascii_case(cursor.node().utf8_text(src.as_bytes()).unwrap())
                {
                    PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword)
                } else {
                    PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Expr)
                };
                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                let select_subexpr = self.format_select_subexpr(cursor, src)?;
                Expr::Sub(Box::new(select_subexpr))
            }
            "parenthesized_expression" => {
                let paren_expr = self.format_paren_expr(cursor, src)?;
                Expr::ParenExpr(Box::new(paren_expr))
            }
            "asterisk_expression" => {
                let asterisk = AsteriskExpr::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap(),
                    Location::new(cursor.node().range()),
                );
                Expr::Asterisk(Box::new(asterisk))
            }
            "conditional_expression" => {
                let cond_expr = self.format_cond_expr(cursor, src)?;
                Expr::Cond(Box::new(cond_expr))
            }
            "function_call" => {
                let func_call = self.format_function_call(cursor, src)?;
                Expr::FunctionCall(Box::new(func_call))
            }
            "TRUE" | "FALSE" | "NULL" => {
                let primary = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                Expr::Primary(Box::new(primary))
            }
            "is_expression" => Expr::Aligned(Box::new(self.format_is_expr(cursor, src)?)),
            "in_expression" => Expr::Aligned(Box::new(self.format_in_expr(cursor, src)?)),
            "type_cast" => Expr::FunctionCall(Box::new(self.format_type_cast(cursor, src)?)),
            "exists_subquery_expression" => {
                Expr::ExistsSubquery(Box::new(self.format_exists_subquery(cursor, src)?))
            }
            "in_subquery_expression" => {
                Expr::Aligned(Box::new(self.format_in_subquery(cursor, src)?))
            }
            "all_some_any_subquery_expression" => {
                Expr::Aligned(Box::new(self.format_all_some_any_subquery(cursor, src)?))
            }
            _ => {
                // todo
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_expr(): unimplemented expression \nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )));
            }
        };

        // バインドパラメータの追加
        if let Some(comment) = head_comment {
            if comment.is_multi_line_comment() && comment.loc().is_next_to(&result.loc()) {
                // 複数行コメントかつ式に隣接していれば、バインドパラメータ
                result.set_head_comment(comment);
            } else {
                // TODO: 隣接していないコメント
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_expr(): (bind parameter) separated comment\nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )));
            }
        }

        Ok(result)
    }

    /// かっこで囲まれた式をフォーマットする
    /// 呼び出し後、cursorはparenthesized_expressionを指す
    pub(crate) fn format_paren_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenExpr, UroboroSQLFmtError> {
        // parenthesized_expression: $ => PREC.unary "(" expression ")"
        // TODO: cursorを引数で渡すよう変更したことにより、tree-sitter-sqlの規則を
        //       _parenthesized_expressionに戻してもよくなったため、修正する

        let loc = Location::new(cursor.node().range());

        // 括弧の前の演算子には未対応

        cursor.goto_first_child();
        // cursor -> "("

        cursor.goto_next_sibling();
        // cursor -> comments | expr

        let mut comment_buf = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> expr

        let expr = self.format_expr(cursor, src)?;

        let mut paren_expr = match expr {
            Expr::ParenExpr(mut paren_expr) => {
                paren_expr.set_loc(loc);
                *paren_expr
            }
            _ => {
                let paren_expr = ParenExpr::new(expr, loc);
                paren_expr
            }
        };

        // 開きかっこと式の間にあるコメントを追加
        for comment in comment_buf {
            paren_expr.add_start_comment(comment);
        }

        // かっこの中の式の最初がバインドパラメータを含む場合でも、comment_bufに読み込まれてしまう
        // そのため、現状ではこの位置のバインドパラメータを考慮していない
        cursor.goto_next_sibling();
        // cursor -> comments | ")"

        // 閉じかっこの前にあるコメントを追加
        while cursor.node().kind() == COMMENT {
            paren_expr.add_comment_to_child(Comment::new(cursor.node(), src))?;
            cursor.goto_next_sibling();
        }

        // tree-sitter-sqlを修正したら削除する
        cursor.goto_parent();
        ensure_kind(cursor, "parenthesized_expression")?;

        Ok(paren_expr)
    }

    /// conflict_targetをフォーマットする
    pub(crate) fn format_conflict_target(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTarget, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // conflict_target =
        //      ( index_column_name  [ COLLATE collation ] [ op_class ] [, ...] ) [ WHERE index_predicate ]
        //      ON CONSTRAINT constraint_name

        if cursor.node().kind() == "ON_CONSTRAINT" {
            //      ON CONSTRAINT constraint_name

            let on_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

            cursor.goto_next_sibling();
            // cursor -> "ON_CONSTRAINT"

            ensure_kind(cursor, "ON_CONSTRAINT")?;
            let constraint_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

            cursor.goto_next_sibling();
            // cursor -> constraint_name

            ensure_kind(cursor, "identifier")?;

            let constraint_name = cursor.node().utf8_text(src.as_bytes()).unwrap();

            cursor.goto_parent();
            ensure_kind(cursor, "conflict_target")?;

            Ok(ConflictTarget::OnConstraint(OnConstraint::new(
                (on_keyword.to_string(), constraint_keyword.to_string()),
                constraint_name.to_string(),
            )))
        } else {
            //      ( index_column_name  [ COLLATE collation ] [ op_class ] [, ...] ) [ WHERE index_predicate ]
            let index_column_name = self.format_conflict_target_column_list(cursor, src)?;
            let mut specify_index_column = SpecifyIndexColumn::new(index_column_name);

            cursor.goto_next_sibling();

            // where句がある場合
            if cursor.node().kind() == "where_clause" {
                let where_clause = self.format_where_clause(cursor, src)?;
                specify_index_column.set_where_clause(where_clause);
            }
            cursor.goto_parent();
            ensure_kind(cursor, "conflict_target")?;

            Ok(ConflictTarget::SpecifyIndexColumn(specify_index_column))
        }
    }

    /// conflict_targetにおけるカラムリストをフォーマットする
    /// "(" カラム名 [COLLATE collation] [op_class] [, ...] ")" という構造になっている
    pub(crate) fn format_conflict_target_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTargetColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(")?;

        // ConflictTargetColumnListの位置
        let mut loc = Location::new(cursor.node().range());
        // ConflictTargetColumnListの要素
        let mut elements = vec![];

        // カラム名 [COLLATE collation] [op_class] [, ...]
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                "identifier" => {
                    let column = cursor.node().utf8_text(src.as_bytes()).unwrap().to_string();
                    let element = ConflictTargetElement::new(column);
                    elements.push(element);
                }
                "," => {
                    continue;
                }
                "COLLATE" => {
                    let collate_keyword =
                        cursor.node().utf8_text(src.as_bytes()).unwrap().to_string();
                    cursor.goto_next_sibling();
                    ensure_kind(cursor, "collation")?;
                    cursor.goto_first_child();
                    ensure_kind(cursor, "identifier")?;
                    let collation = cursor.node().utf8_text(src.as_bytes()).unwrap().to_string();

                    // elementsの最後の要素にCOLLATEをセット
                    elements
                        .last_mut()
                        .unwrap()
                        .set_collate(Collate::new(collate_keyword, collation));
                    cursor.goto_parent();
                }
                "op_class" => {
                    cursor.goto_first_child();
                    ensure_kind(cursor, "identifier")?;
                    let op_class = cursor.node().utf8_text(src.as_bytes()).unwrap().to_string();

                    // elementsの最後の要素にop_classをセット
                    elements.last_mut().unwrap().set_op_class(op_class);
                    cursor.goto_parent();
                }
                ")" => break,
                _ => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_conflict_target_column_list(): Unexpected node\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )));
                }
            }
        }
        Ok(ConflictTargetColumnList::new(elements, loc))
    }

    /// カラムリストをColumnListで返す
    /// カラムリストは "(" 式 ["," 式 ...] ")"という構造になっている
    pub(crate) fn format_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(")?;

        // ColumnListの位置
        let mut loc = Location::new(cursor.node().range());

        cursor.goto_next_sibling();

        let mut exprs = vec![self.format_expr(cursor, src)?.to_aligned()];

        // カンマ区切りの式
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                "," => {
                    cursor.goto_next_sibling();
                    exprs.push(self.format_expr(cursor, src)?.to_aligned());
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
                        return Err(UroboroSQLFmtError::UnimplementedError(format!(
                            "format_column_list(): Unexpected comment\nnode_kind: {}\n{:#?}",
                            cursor.node().kind(),
                            cursor.node().range(),
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_column_list(): Unexpected node\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )));
                }
            }
        }

        Ok(ColumnList::new(exprs, loc))
    }

    /// IS式のフォーマットを行う。
    /// 結果を AlignedExpr で返す。
    fn format_is_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let lhs = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();
        ensure_kind(cursor, "IS")?;
        let op = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
        cursor.goto_next_sibling();

        // 右辺は "NOT" から始まる場合がある。
        // TODO: tree-sitter-sql では、右辺に distinct_from が現れるケースがあり、それには対応していない。
        let rhs = if cursor.node().kind() == "NOT" {
            let not_str = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
            let mut loc = Location::new(cursor.node().range());
            cursor.goto_next_sibling();

            let operand = self.format_expr(cursor, src)?;
            loc.append(operand.loc());
            Expr::Unary(Box::new(UnaryExpr::new(not_str, operand, loc)))
        } else {
            self.format_expr(cursor, src)?
        };

        let mut aligned = AlignedExpr::new(lhs, false);
        aligned.add_rhs(op, rhs);

        cursor.goto_parent();
        ensure_kind(cursor, "is_expression")?;

        Ok(aligned)
    }

    /// IN式に対して、AlignedExprを返す。
    /// IN式は、(expr NOT? IN tuple) という構造をしている。
    fn format_in_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let lhs = self.format_expr(cursor, src)?;
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

        ensure_kind(cursor, "IN")?;
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

        ensure_kind(cursor, "tuple")?;
        // body のネスト分と、開きかっこのネストで、二重にネストさせる。
        // TODO: body の走査に入った時点で、ネストするべきかもしれない。

        cursor.goto_first_child();
        let mut column_list = self.format_column_list(cursor, src)?;
        cursor.goto_parent();

        ensure_kind(cursor, "tuple")?;

        if let Some(comment) = bind_param {
            if comment.is_multi_line_comment() && comment.loc().is_next_to(&column_list.loc()) {
                column_list.set_head_comment(comment);
            } else {
                return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                    "format_in_expr(): unexpected comment\n{:?}",
                    comment
                )));
            }
        }

        let rhs = Expr::ColumnList(Box::new(column_list));

        let mut aligned = AlignedExpr::new(lhs, false);
        aligned.add_rhs(op, rhs);

        cursor.goto_parent();
        ensure_kind(cursor, "in_expression")?;

        Ok(aligned)
    }
}

/// 引数の文字列が比較演算子かどうかを判定する
pub(crate) fn is_comp_op(op_str: &str) -> bool {
    matches!(
        op_str,
        "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
    )
}
