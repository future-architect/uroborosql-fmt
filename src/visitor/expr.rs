mod aliasable;
mod assignment;
mod binary;
mod boolean;
mod column_list;
mod cond;
mod conflict_target;
mod function;
mod in_expr;
mod is;
mod paren;
mod subquery;

use tree_sitter::TreeCursor;

use crate::{cst::*, error::UroboroSQLFmtError, util::convert_identifier_case};

use super::{ensure_kind, Visitor, COMMENT};

impl Visitor {
    /// 式のフォーマットを行う。
    /// cursorがコメントを指している場合、バインドパラメータであれば結合して返す。
    /// 式の初めにバインドパラメータが現れた場合、式の本体は隣の兄弟ノードになる。
    /// 呼び出し後、cursorは式の本体のノードを指す
    pub(crate) fn visit_expr(
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
                            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                "visit_expr: ERROR node appeared \n{:?}",
                                cursor.node().range()
                            )));
                        }
                        _ => dotted_name.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary =
                    PrimaryExpr::new(convert_identifier_case(&dotted_name), Location::new(range));

                // cursorをdotted_nameに戻す
                cursor.goto_parent();
                ensure_kind(cursor, "dotted_name")?;

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => self.visit_binary_expr(cursor, src)?,
            "between_and_expression" => {
                Expr::Aligned(Box::new(self.visit_between_and_expression(cursor, src)?))
            }
            "boolean_expression" => self.visit_bool_expr(cursor, src)?,
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
                let select_subexpr = self.visit_select_subexpr(cursor, src)?;
                Expr::Sub(Box::new(select_subexpr))
            }
            "parenthesized_expression" => {
                let paren_expr = self.visit_paren_expr(cursor, src)?;
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
                let cond_expr = self.visit_cond_expr(cursor, src)?;
                Expr::Cond(Box::new(cond_expr))
            }
            "function_call" => {
                let func_call = self.visit_function_call(cursor, src)?;
                Expr::FunctionCall(Box::new(func_call))
            }
            "TRUE" | "FALSE" | "NULL" => {
                let primary = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                Expr::Primary(Box::new(primary))
            }
            "is_expression" => Expr::Aligned(Box::new(self.visit_is_expr(cursor, src)?)),
            "in_expression" => Expr::Aligned(Box::new(self.visit_in_expr(cursor, src)?)),
            "type_cast" => Expr::FunctionCall(Box::new(self.visit_type_cast(cursor, src)?)),
            "exists_subquery_expression" => {
                Expr::ExistsSubquery(Box::new(self.visit_exists_subquery(cursor, src)?))
            }
            "in_subquery_expression" => {
                Expr::Aligned(Box::new(self.visit_in_subquery(cursor, src)?))
            }
            "all_some_any_subquery_expression" => {
                Expr::Aligned(Box::new(self.visit_all_some_any_subquery(cursor, src)?))
            }
            _ => {
                // todo
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_expr(): unimplemented expression \nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )));
            }
        };

        // バインドパラメータの追加
        if let Some(comment) = head_comment {
            if comment.is_block_comment() && comment.loc().is_next_to(&result.loc()) {
                // 複数行コメントかつ式に隣接していれば、バインドパラメータ
                result.set_head_comment(comment);
            } else {
                // TODO: 隣接していないコメント
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_expr(): (bind parameter) separated comment\nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )));
            }
        }

        Ok(result)
    }
}

/// 引数の文字列が比較演算子かどうかを判定する
pub(crate) fn is_comp_op(op_str: &str) -> bool {
    matches!(
        op_str,
        "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
    )
}
