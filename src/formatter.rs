use itertools::{repeat_n, Itertools};
use tree_sitter::{Node, Point, Range};

const TAB_SIZE: usize = 4; // タブ幅

const COMPLEMENT_AS: bool = true; // AS句がない場合に自動的に補完する

const TRIM_BIND_PARAM: bool = false; // バインド変数の中身をトリムする

pub const DEBUG_MODE: bool = false; // デバッグモード

pub const COMMENT: &str = "comment";

use crate::cst::*;

/// インデントの深さや位置をそろえるための情報を保持する構造体
struct FormatterState {
    pub depth: usize,
}

pub struct Formatter {
    state: FormatterState,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub fn new() -> Formatter {
        Formatter {
            state: FormatterState { depth: 0 },
        }
    }

    /// sqlソースファイルをフォーマットし、bufに入れる
    pub fn format_sql(&mut self, node: Node, src: &str) -> Statement {
        self.format_source(node, src)
    }

    // ネストを1つ深くする
    fn nest(&mut self) {
        self.state.depth += 1;
    }

    // ネストを1つ浅くする
    fn unnest(&mut self) {
        self.state.depth -= 1;
    }

    fn format_source(&mut self, node: Node, src: &str) -> Statement {
        // source_file -> _statement*

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let stmt_node = cursor.node();

            // 現状はselect_statementのみ
            // 文が増えたらマッチ式で分岐させる
            let mut stmt = self.format_select_stmt(stmt_node, src);

            if cursor.goto_next_sibling() && cursor.node().kind() == COMMENT {
                let comment_loc = Location::new(cursor.node().range());

                //同じ行の場合
                if comment_loc.is_same_line(&stmt.loc().unwrap()) {
                    stmt.add_comment_to_child(Comment::new(
                        cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                        comment_loc,
                    ));
                } else {
                    //違う行の場合
                }
            }
            stmt
        } else {
            todo!()
        }
    }

    // SELECT文
    fn format_select_stmt(&mut self, node: Node, src: &str) -> Statement {
        /*
            _select_statement ->
                select_clause
                from_clause?
                where_clause?
        */

        let mut statement = Statement::default();

        let mut cursor = node.walk(); // cursor -> select_statement
        if cursor.goto_first_child() {
            // cursor -> select_clause
            let select_clause_node = cursor.node();

            statement.add_clause(self.format_select_clause(select_clause_node, src));
        }

        loop {
            // 次の兄弟へ移動
            // select_statementの子供がいなくなったら終了
            if !cursor.goto_next_sibling() {
                break;
            }

            let clause_node = cursor.node();
            // println!("{}", clause_node.kind());

            match clause_node.kind() {
                "from_clause" => {
                    if cursor.goto_first_child() {
                        // cursor -> FROM
                        let from_node = cursor.node();
                        let mut clause = Clause::new(
                            "FROM".to_string(),
                            Location::new(from_node.range()),
                            self.state.depth,
                        );

                        let mut separated_lines = SeparatedLines::new(self.state.depth, ",", true);

                        // commaSep
                        if cursor.goto_next_sibling() {
                            // cursor -> _aliasable_expression
                            let expr_node = cursor.node();

                            separated_lines.add_expr(self.format_aliasable_expr(expr_node, src));

                            while cursor.goto_next_sibling() {
                                // cursor -> , または cursor -> _aliasable_expression
                                let child_node = cursor.node();

                                match child_node.kind() {
                                    "," => continue,
                                    COMMENT => {
                                        let comment_loc = Location::new(child_node.range());

                                        //同じ行の場合
                                        if comment_loc.is_same_line(&separated_lines.loc().unwrap())
                                        {
                                            separated_lines.add_comment_to_child(Comment::new(
                                                child_node
                                                    .utf8_text(src.as_bytes())
                                                    .unwrap()
                                                    .to_string(),
                                                comment_loc,
                                            ));
                                        } else {
                                            //違う行の場合
                                        }
                                    }
                                    _ => {
                                        let alias = self.format_aliasable_expr(child_node, src);
                                        separated_lines.add_expr(alias);
                                    }
                                };
                            }
                        }

                        cursor.goto_parent();

                        clause.set_body(Body::SepLines(separated_lines));

                        statement.add_clause(clause);
                    }
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    if cursor.goto_first_child() {
                        // cursor -> WHERE
                        let where_node = cursor.node();
                        let mut clause = Clause::new(
                            "WHERE".to_string(),
                            Location::new(where_node.range()),
                            self.state.depth,
                        );

                        cursor.goto_next_sibling();
                        // cursor -> _expression

                        let expr_node = cursor.node();
                        let expr = self.format_expr(expr_node, src);

                        let body = match expr {
                            Expr::Aligned(aligned) => {
                                let mut separated_lines =
                                    SeparatedLines::new(self.state.depth, "", false);
                                separated_lines.add_expr(*aligned);
                                Body::SepLines(separated_lines)
                            }
                            Expr::Primary(_) => {
                                todo!();
                            }
                            Expr::Boolean(boolean) => Body::BooleanExpr(*boolean),
                            Expr::SelectSub(_select_sub) => todo!(),
                            Expr::ParenExpr(paren_expr) => Body::ParenExpr(*paren_expr),
                            Expr::Asterisk(_asterisk) => todo!(),
                        };

                        cursor.goto_parent();

                        clause.set_body(body);

                        statement.add_clause(clause);
                    }
                }
                COMMENT => {
                    let comment_loc = Location::new(clause_node.range());

                    //同じ行の場合
                    if comment_loc.is_same_line(&statement.loc().unwrap()) {
                        statement.add_comment_to_child(Comment::new(
                            clause_node.utf8_text(src.as_bytes()).unwrap().to_string(),
                            comment_loc,
                        ));
                    } else {
                        //違う行の場合
                    }
                }
                _ => {
                    break;
                }
            }
        }

        statement
    }

    // SELECT句
    fn format_select_clause(&mut self, node: Node, src: &str) -> Clause {
        /*
            select_clause ->
                "SELECT"
                select_clause_body
        */
        let mut cursor = node.walk(); // cursor -> select_clause

        let mut clause = Clause::new(
            "SELECT".to_string(),
            Location::new(node.range()),
            self.state.depth,
        );

        if cursor.goto_first_child() {
            // cursor -> SELECT
            // SELECTを読み飛ばす(コメントを考える際に変更予定)

            // if self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
            cursor.goto_next_sibling();
            // cursor -> select_caluse_body

            let body = self.format_select_clause_body(cursor.node(), src);
            clause.set_body(Body::SepLines(body));
        }

        clause
    }

    // SELECT句の本体をSeparatedLinesで返す
    fn format_select_clause_body(&mut self, node: Node, src: &str) -> SeparatedLines {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        let mut cursor = node.walk(); // cursor -> select_clause_body

        cursor.goto_first_child();
        // cursor -> _aliasable_expression

        let expr_node = cursor.node();

        let mut separated_lines = SeparatedLines::new(self.state.depth, ",", false);

        let aligned = self.format_aliasable_expr(expr_node, src);
        separated_lines.add_expr(aligned);

        // (',' _aliasable_expression)*
        // while self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
        while cursor.goto_next_sibling() {
            // cursor -> , または cursor -> _aliasable_expression
            let child_node = cursor.node();
            match child_node.kind() {
                "," => continue,
                COMMENT => {
                    separated_lines.add_comment_to_child(Comment::new(
                        child_node.utf8_text(src.as_bytes()).unwrap().to_string(),
                        Location::new(child_node.range()),
                    ));
                }
                _ => {
                    let aligned = self.format_aliasable_expr(child_node, src);
                    separated_lines.add_expr(aligned);
                }
            }
        }

        separated_lines
    }

    // エイリアス可能な式
    fn format_aliasable_expr(&mut self, node: Node, src: &str) -> AlignedExpr {
        /*
            _aliasable_expression ->
                alias | _expression

            alias ->
                _expression
                "AS"?
                identifier
                << 未対応!! "(" identifier ("," identifier)* ")" >>
        */
        match node.kind() {
            "alias" => {
                let mut cursor = node.walk();
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let lhs_expr = self.format_expr(cursor.node(), src);
                let lhs_expr_loc = lhs_expr.loc();

                let mut aligned = AlignedExpr::new(lhs_expr, lhs_expr_loc, true);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() {
                    // cursor -> "AS"?

                    // ASが存在する場合は読み飛ばす
                    if cursor.node().kind() == "AS" {
                        cursor.goto_next_sibling();
                    }

                    //右辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    if cursor.node().kind() == "identifier" {
                        let rhs = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        let rhs_expr =
                            PrimaryExpr::new(rhs.to_string(), Location::new(cursor.node().range()));
                        aligned.add_rhs("AS".to_string(), Expr::Primary(Box::new(rhs_expr)));
                    }
                }
                aligned
            }
            _ => {
                // _expression
                let expr_node = node;
                let expr = self.format_expr(expr_node, src);
                let expr_loc = expr.loc();

                AlignedExpr::new(expr, expr_loc, true)
            }
        }
    }

    // 引数の文字列が比較演算子かどうかを判定する
    fn is_comp_op(op_str: &str) -> bool {
        matches!(
            op_str,
            "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
        )
    }

    // 式
    fn format_expr(&mut self, node: Node, src: &str) -> Expr {
        let mut cursor = node.walk();

        match cursor.node().kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                // cursor -> dotted_name

                let range = node.range();

                cursor.goto_first_child();

                // cursor -> identifier

                let mut dotted_name = String::new();

                let id_node = cursor.node();
                dotted_name.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                // while self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src) {
                while cursor.goto_next_sibling() {
                    // cursor -> . または cursor -> identifier
                    match cursor.node().kind() {
                        "." => dotted_name.push('.'),
                        _ => dotted_name.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary = PrimaryExpr::new(dotted_name, Location::new(range));

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => {
                // cursor -> binary_expression

                cursor.goto_first_child();
                // cursor -> _expression

                // 左辺
                let lhs_node = cursor.node();
                let lhs_expr = self.format_expr(lhs_node, src);

                // self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src);
                cursor.goto_next_sibling();
                // cursor -> op (e.g., "+", "-", "=", ...)

                // 演算子
                let op_node = cursor.node();
                let op_str = op_node.utf8_text(src.as_ref()).unwrap();

                cursor.goto_next_sibling();
                // cursor -> _expression

                let mut head_comment: Option<Comment> = None;
                if cursor.node().kind() == COMMENT {
                    let comment_loc = Location::new(cursor.node().range());
                    head_comment = Some(Comment::new(
                        cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                        comment_loc,
                    ));

                    cursor.goto_next_sibling();
                }

                // 右辺
                let rhs_node = cursor.node();
                let mut rhs_expr = self.format_expr(rhs_node, src);

                match head_comment {
                    Some(comment) => match &mut rhs_expr {
                        Expr::Aligned(_) => todo!(),
                        Expr::Primary(primary) => primary.set_head_comment(comment),
                        Expr::Boolean(_) => todo!(),
                        Expr::SelectSub(_) => todo!(),
                        Expr::ParenExpr(_) => todo!(),
                        Expr::Asterisk(_) => todo!(),
                    },
                    None => (),
                }

                if Self::is_comp_op(op_str) {
                    // 比較演算子 -> AlignedExpr
                    let lhs_loc = lhs_expr.loc();
                    let mut aligned = AlignedExpr::new(lhs_expr, lhs_loc, false);
                    aligned.add_rhs(op_str.to_string(), rhs_expr);

                    Expr::Aligned(Box::new(aligned))
                } else {
                    // 比較演算子でない -> PrimaryExpr
                    // e.g.,) 1 + 1
                    match lhs_expr {
                        Expr::Primary(mut lhs) => {
                            lhs.add_element(op_str);
                            match rhs_expr {
                                Expr::Primary(rhs) => lhs.append(*rhs),
                                _ => {
                                    // 右辺が複数行の場合
                                    todo!()
                                }
                            }
                            Expr::Primary(lhs)
                        }
                        _ => {
                            // 左辺が複数行の場合
                            todo!()
                        }
                    }
                }
            }
            "boolean_expression" => self.format_bool_expr(node, src),
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let primary = PrimaryExpr::new(
                    node.utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(node.range()),
                );

                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                self.nest();
                let select_subexpr = self.format_select_subexpr(node, src);
                self.unnest();
                Expr::SelectSub(Box::new(select_subexpr))
            }
            "parenthesized_expression" => {
                let paren_expr = self.format_paren_expr(node, src);
                Expr::ParenExpr(Box::new(paren_expr))
            }
            "asterisk_expression" => {
                let asterisk = AsteriskExpr::new(
                    node.utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(node.range()),
                );
                Expr::Asterisk(Box::new(asterisk))
            }
            _ => {
                eprintln!(
                    "format_expr(): unimplemented expression {}, {:#?}",
                    cursor.node().kind(),
                    cursor.node().range()
                );
                todo!()
            }
        }
    }

    fn format_bool_expr(&mut self, node: Node, src: &str) -> Expr {
        /*
        boolean_expression: $ =>
            choice(
            prec.left(PREC.unary, seq(kw("NOT"), $._expression)),
            prec.left(PREC.and, seq($._expression, kw("AND"), $._expression)),
            prec.left(PREC.or, seq($._expression, kw("OR"), $._expression)),
        ),
         */

        let mut boolean_expr = BooleanExpr::new(self.state.depth, "-");

        let mut cursor = node.walk();

        cursor.goto_first_child();

        if cursor.node().kind() == "NOT" {
            todo!();
        } else {
            let left = self.format_expr(cursor.node(), src);
            match left {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
                Expr::ParenExpr(paren_expr) => {
                    let loc = paren_expr.loc();
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), loc, false);
                    boolean_expr.add_expr(aligned);
                }
                Expr::Asterisk(_) => todo!(),
            }

            cursor.goto_next_sibling();

            if cursor.node().kind() == COMMENT {
                let comment_loc = Location::new(cursor.node().range());
                boolean_expr.add_comment_to_child(Comment::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    comment_loc,
                ));
                cursor.goto_next_sibling();
            }

            let sep = cursor.node().kind();
            boolean_expr.set_default_separator(sep.to_string());

            cursor.goto_next_sibling();
            let right = self.format_expr(cursor.node(), src);

            match right {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
                Expr::ParenExpr(paren_expr) => {
                    let loc = paren_expr.loc();
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), loc, false);
                    boolean_expr.add_expr(aligned);
                }
                Expr::Asterisk(_) => todo!(),
            }
        }
        Expr::Boolean(Box::new(boolean_expr))
    }

    fn format_select_subexpr(&mut self, node: Node, src: &str) -> SelectSubExpr {
        // select_subexpression -> "(" select_statement ")"

        let loc = Location::new(node.range());

        let mut cursor = node.walk(); // cursor -> select_subexpression

        cursor.goto_first_child();
        // cursor -> (
        // 将来的には、かっこの数を数えるかもしれない

        cursor.goto_next_sibling();
        // cursor -> select_statement

        self.nest();
        let select_stmt_node = cursor.node();
        let select_stmt = self.format_select_stmt(select_stmt_node, src);
        self.unnest();

        cursor.goto_next_sibling();
        // cursor -> )

        SelectSubExpr::new(select_stmt, loc, self.state.depth)
    }

    fn format_paren_expr(&mut self, node: Node, src: &str) -> ParenExpr {
        // parenthesized_expression: $ => PREC.unary "(" expression ")"
        let mut cursor = node.walk();

        let loc = Location::new(cursor.node().range());

        // 括弧の前の演算子には未対応

        cursor.goto_first_child();
        //cursor -> "("

        cursor.goto_next_sibling();
        //cursor -> expr

        // exprがparen_exprならネストしない
        let is_nest = match cursor.node().kind() {
            "parenthesized_expression" => false,
            _ => true,
        };

        if is_nest {
            self.nest();
        }

        let expr = self.format_expr(cursor.node(), src);

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

        cursor.goto_next_sibling();
        //cursor -> ")"

        if cursor.node().kind() == COMMENT {
            let comment_loc = Location::new(cursor.node().range());
            paren_expr.add_comment_to_child(Comment::new(
                cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                comment_loc,
            ));
            cursor.goto_next_sibling();
        }

        paren_expr
    }
}
