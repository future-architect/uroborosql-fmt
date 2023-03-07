mod binary;
mod boolean;
mod cond;
mod function;
mod subquery;

use tree_sitter::TreeCursor;

use crate::{cst::*, util::format_keyword};

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

        match cursor.node().kind() {
            "alias" => {
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let lhs_expr = self.format_expr(cursor, src)?;

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

                    let rhs_expr = PrimaryExpr::with_node(cursor.node(), src);
                    aligned.add_rhs(format_keyword("AS"), Expr::Primary(Box::new(rhs_expr)));
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
                        _ => dotted_name.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary = PrimaryExpr::new(dotted_name, Location::new(range));

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
                let primary = PrimaryExpr::with_node(cursor.node(), src);
                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                self.nest();
                let select_subexpr = self.format_select_subexpr(cursor, src)?;
                self.unnest();
                Expr::SelectSub(Box::new(select_subexpr))
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
                let primary = PrimaryExpr::with_node(cursor.node(), src);
                Expr::Primary(Box::new(primary))
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

        // exprがparen_exprならネストしない
        let is_nest = !matches!(cursor.node().kind(), "parenthesized_expression");

        if is_nest {
            self.nest();
        }

        let expr = self.format_expr(cursor, src)?;

        let mut paren_expr = match expr {
            Expr::ParenExpr(mut paren_expr) => {
                paren_expr.set_loc(loc);
                *paren_expr
            }
            _ => {
                let paren_expr = ParenExpr::new(expr, loc, self.state.depth);
                self.unnest();
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

    /// カラムリストをColumnListで返す
    /// カラムリストはVALUES句、SET句で現れ、"(" 式 ["," 式 ...] ")"という構造になっている
    pub(crate) fn format_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(")?;
        let mut loc = Location::new(cursor.node().range());

        let mut exprs = vec![];
        // commaSep1(_expression)
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                "," => continue,
                ")" => break,
                COMMENT => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_column_list(): Unexpected comment\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )))
                }
                _ => {
                    exprs.push(self.format_expr(cursor, src)?);
                }
            }
        }

        Ok(ColumnList::new(exprs, loc))
    }
}

/// 引数の文字列が比較演算子かどうかを判定する
pub(crate) fn is_comp_op(op_str: &str) -> bool {
    matches!(
        op_str,
        "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
    )
}
