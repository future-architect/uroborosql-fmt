use tree_sitter::{Node, TreeCursor};

pub(crate) const COMMENT: &str = "comment";

use crate::cst::*;

/// インデントの深さや位置をそろえるための情報を保持する構造体
struct FormatterState {
    pub(crate) depth: usize,
}

pub(crate) struct Formatter {
    state: FormatterState,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub(crate) fn new() -> Formatter {
        Formatter {
            state: FormatterState { depth: 0 },
        }
    }

    /// sqlソースファイルをフォーマット用構造体に変形する
    pub(crate) fn format_sql(&mut self, node: Node, src: &str) -> Vec<Statement> {
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

    fn format_source(&mut self, node: Node, src: &str) -> Vec<Statement> {
        // source_file -> _statement*
        let mut source: Vec<Statement> = vec![];

        let mut cursor = node.walk();

        if !cursor.goto_first_child() {
            // source_fileに子供がない
            todo!()
        }

        // ソースファイル先頭のコメントを保存するバッファ
        let mut comment_buf: Vec<Comment> = vec![];

        loop {
            let stmt_node = cursor.node();

            match stmt_node.kind() {
                "select_statement" => {
                    let mut stmt = self.format_select_stmt(stmt_node, src);

                    // コメントが以前にあれば先頭に追加
                    comment_buf
                        .iter()
                        .cloned()
                        .for_each(|c| stmt.add_comment(c));
                    comment_buf.clear();

                    source.push(stmt);
                }
                COMMENT => {
                    let comment = Comment::new(stmt_node, src);

                    if let Some(last_stmt) = source.last_mut() {
                        // すでにstatementがある場合、末尾に追加
                        last_stmt.add_comment_to_child(comment);
                    } else {
                        // まだstatementがない場合、バッファに詰めておく
                        comment_buf.push(comment);
                    }
                }
                _ => unimplemented!(),
            }

            if !cursor.goto_next_sibling() {
                // 次の子供がいない場合、終了
                break;
            }
        }

        source
    }

    // SELECT文
    fn format_select_stmt(&mut self, node: Node, src: &str) -> Statement {
        /*
            _select_statement ->
                select_clause
                from_clause?
                where_clause?
        */

        let mut statement = Statement::new(self.state.depth);

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
                            while cursor.node().kind() == COMMENT {
                                let comment = Comment::new(cursor.node(), src);
                                clause.add_comment_to_child(comment);
                                cursor.goto_next_sibling();
                            }

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
                                                child_node, src,
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

                        while cursor.node().kind() == COMMENT {
                            let comment = Comment::new(cursor.node(), src);
                            clause.add_comment_to_child(comment);
                            cursor.goto_next_sibling();
                        }

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
                            Expr::ParenExpr(paren_expr) => {
                                // paren_exprをaligned_exprでラップする
                                let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), false);

                                // Bodyを返すため、separated_linesに格納
                                let mut separated_lines =
                                    SeparatedLines::new(self.state.depth, "", false);
                                separated_lines.add_expr(aligned);

                                Body::SepLines(separated_lines)
                            }
                            Expr::Asterisk(_asterisk) => todo!(),
                            _ => unimplemented!(),
                        };

                        cursor.goto_parent();

                        clause.set_body(body);

                        statement.add_clause(clause);
                    }
                }
                "UNION" | "INTERSECT" | "EXCEPT" => {
                    // 演算の文字列(e.g., "INTERSECT", "UNION ALL", ...)
                    let mut combining_op = String::from(cursor.node().kind());

                    // 演算のソースコード上での位置
                    let mut loc = Location::new(cursor.node().range());

                    cursor.goto_next_sibling();
                    // cursor -> (ALL | DISTINCT) | select_statement

                    if matches!(cursor.node().kind(), "ALL" | "DISTINCT") {
                        // ALL または DISTINCT を追加する
                        combining_op.push(' ');
                        combining_op.push_str(cursor.node().kind());
                        loc.append(Location::new(cursor.node().range()));
                        cursor.goto_next_sibling();
                    }
                    // cursor -> comments | select_statement

                    // 演算子のみからなる句を追加
                    let combining_clause = Clause::new(combining_op, loc, self.state.depth);
                    statement.add_clause(combining_clause);

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        statement.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // 副問い合わせを計算
                    let select_stmt = self.format_select_stmt(cursor.node(), src);
                    select_stmt
                        .get_clauses()
                        .iter()
                        .for_each(|clause| statement.add_clause(clause.to_owned()));
                }
                COMMENT => statement.add_comment_to_child(Comment::new(clause_node, src)),
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

            cursor.goto_next_sibling();
            // cursor -> comments | select_clause_body

            while cursor.node().kind() == COMMENT {
                let comment_node = cursor.node();

                let comment = Comment::new(comment_node, src);

                // _SQL_ID_かどうかをチェックする
                if comment.is_sql_id_comment() {
                    clause.set_sql_id(comment);
                } else {
                    clause.add_comment_to_child(comment)
                }

                cursor.goto_next_sibling();
            }
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
                    separated_lines.add_comment_to_child(Comment::new(child_node, src));
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
                let lhs_node = cursor.node();
                let lhs_expr = self.format_expr(lhs_node, src);

                let mut aligned = AlignedExpr::new(lhs_expr, true);

                // ("AS"? identifier)?
                if goto_next_to_expr(&mut cursor) {
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
                            panic!("unexpected syntax")
                        } else {
                            // 行末コメント
                            aligned.set_lhs_trailing_comment(comment);
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

                AlignedExpr::new(expr, true)
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

    /// 式のフォーマットを行う。
    /// コメントを与えた場合、バインドパラメータであれば結合して返す。
    /// 式の初めにバインドパラメータが現れた場合、式の本体は隣の兄弟ノードになる。
    /// その場合、呼び出し元のカーソルはバインドパラメータを指しているため、1度`cursor.goto_next_sibling()`を
    /// 呼び出しただけでは、式の次のノードにカーソルを移動させることができない。
    /// そのため、式の次のノードにカーソルを移動させる際は`goto_next_to_expr()`を使用する。
    fn format_expr(&mut self, node: Node, src: &str) -> Expr {
        let mut cursor = node.walk();

        // バインドパラメータをチェック
        let head_comment = if cursor.node().kind() == COMMENT {
            let comment_node = cursor.node();
            let next_sibling_node = node
                .next_sibling()
                .expect("this expression has only comment, no body.");
            cursor = next_sibling_node.walk();
            // cursor -> _expression
            // (式の直前に複数コメントが来る場合は想定していない)
            Some(Comment::new(comment_node, src))
        } else {
            None
        };

        match cursor.node().kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                // cursor -> dotted_name

                let range = cursor.node().range();

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

                let mut primary = PrimaryExpr::new(dotted_name, Location::new(range));
                if let Some(comment) = head_comment {
                    if comment.is_multi_line_comment() && comment.loc().is_next_to(&primary.loc()) {
                        // 複数行コメントかつ式に隣接していれば、バインドパラメータ
                        primary.set_head_comment(comment);
                    } else {
                        // TODO: 隣接していないコメント
                        todo!()
                    }
                }

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => {
                // cursor -> binary_expression

                cursor.goto_first_child();
                // cursor -> _expression

                // 左辺
                let lhs_node = cursor.node();
                let lhs_expr = self.format_expr(lhs_node, src);

                goto_next_to_expr(&mut cursor);
                // cursor -> op (e.g., "+", "-", "=", ...)

                // 演算子
                let op_node = cursor.node();
                let op_str = op_node.utf8_text(src.as_ref()).unwrap();

                cursor.goto_next_sibling();
                // cursor -> _expression

                // 右辺
                let rhs_node = cursor.node();
                let rhs_expr = self.format_expr(rhs_node, src);

                if Self::is_comp_op(op_str) {
                    // 比較演算子 -> AlignedExpr
                    let mut aligned = AlignedExpr::new(lhs_expr, false);
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
            "between_and_expression" => {
                Expr::Aligned(Box::new(self.format_between_and_expression(node, src)))
            }
            "boolean_expression" => self.format_bool_expr(node, src),
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let mut primary = PrimaryExpr::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(cursor.node().range()),
                );

                if let Some(comment) = head_comment {
                    if comment.is_multi_line_comment() && comment.loc().is_next_to(&primary.loc()) {
                        // 複数行コメントかつ式に隣接していれば、バインドパラメータ
                        primary.set_head_comment(comment);
                    } else {
                        // TODO: 隣接していないコメント
                        todo!(
                            "\ncomment: {:?}\nprimary: {:?}",
                            comment.loc(),
                            primary.loc()
                        )
                    }
                }

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
            "conditional_expression" => {
                let cond_expr = self.format_cond_expr(node, src);
                Expr::Cond(Box::new(cond_expr))
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
            // and or
            let left = self.format_expr(cursor.node(), src);
            match left {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
                Expr::ParenExpr(paren_expr) => {
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), false);
                    boolean_expr.add_expr(aligned);
                }
                Expr::Asterisk(_) => todo!(),
                _ => unimplemented!(),
            }

            goto_next_to_expr(&mut cursor);

            while cursor.node().kind() == COMMENT {
                boolean_expr.add_comment_to_child(Comment::new(cursor.node(), src));
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
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), false);
                    boolean_expr.add_expr(aligned);
                }
                Expr::Asterisk(_) => todo!(),
                _ => unimplemented!(),
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
        self.nest();

        cursor.goto_next_sibling();
        // cursor -> comments | select_statement

        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> select_statement
        let select_stmt_node = cursor.node();
        let mut select_stmt = self.format_select_stmt(select_stmt_node, src);

        // select_statementの前にコメントがあった場合、コメントを追加
        comment_buf
            .into_iter()
            .for_each(|c| select_stmt.add_comment(c));

        cursor.goto_next_sibling();
        // cursor -> comments | )

        while cursor.node().kind() == COMMENT {
            // 閉じかっこの直前にコメントが来る場合
            let comment = Comment::new(cursor.node(), src);
            select_stmt.add_comment_to_child(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> )
        self.unnest();

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
        //cursor -> comments | expr

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

        // 開きかっこと式の間にあるコメントを追加
        for comment in comment_buf {
            paren_expr.add_start_comment(comment);
        }

        // かっこの中の式の最初がバインドパラメータを含む場合でも、comment_bufに読み込まれてしまう
        // そのため、現状ではこの位置のバインドパラメータを考慮せず、goto_next_to_expr()を使用していない
        cursor.goto_next_sibling();
        // cursor -> comments | ")"

        // 閉じかっこの前にあるコメントを追加
        while cursor.node().kind() == COMMENT {
            paren_expr.add_comment_to_child(Comment::new(cursor.node(), src));
            cursor.goto_next_sibling();
        }

        paren_expr
    }

    fn format_cond_expr(&mut self, node: Node, src: &str) -> CondExpr {
        // conditional_expression ->
        //     "CASE"
        //     ("WHEN" expression "THEN" expression)*
        //     ("ELSE" expression)?
        //     "END"

        let mut cursor = node.walk();
        let mut cond_expr = CondExpr::new(Location::new(node.range()), self.state.depth);

        // CASE, WHEN(, THEN, ELSE)キーワードの分で2つネストが深くなる
        // TODO: ネストの深さの計算をrender()メソッドで行う変更
        self.nest();
        self.nest();

        cursor.goto_first_child();
        // cursor -> "CASE"

        while cursor.goto_next_sibling() {
            // cursor -> "WHEN" || "ELSE" || "END"
            let kw_node = cursor.node();

            match kw_node.kind() {
                "WHEN" => {
                    let mut when_clause = Clause::new(
                        "WHEN".to_string(),
                        Location::new(kw_node.range()),
                        self.state.depth,
                    );

                    cursor.goto_next_sibling();
                    // cursor -> comment | _expression

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        when_clause.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // cursor -> _expression

                    let when_expr_node = cursor.node();
                    let when_expr = self.format_expr(when_expr_node, src);
                    when_clause.set_body(Body::new_body_with_expr(when_expr, self.state.depth));

                    goto_next_to_expr(&mut cursor);
                    // cursor -> comment || "THEN"

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        when_clause.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // cursor -> "THEN"
                    let mut then_clause = Clause::new(
                        "THEN".to_string(),
                        Location::new(cursor.node().range()),
                        self.state.depth,
                    );

                    cursor.goto_next_sibling();
                    // cursor -> comment || _expression

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        then_clause.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // cursor -> _expression

                    let then_expr_node = cursor.node();
                    let then_expr = self.format_expr(then_expr_node, src);
                    then_clause.set_body(Body::new_body_with_expr(then_expr, self.state.depth));

                    cond_expr.add_when_then_clause(when_clause, then_clause);
                }
                "ELSE" => {
                    let mut else_clause = Clause::new(
                        "ELSE".to_string(),
                        Location::new(cursor.node().range()),
                        self.state.depth,
                    );

                    cursor.goto_next_sibling();
                    // cursor -> comment || _expression

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        else_clause.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // cursor -> _expression

                    let else_expr_node = cursor.node();
                    let else_expr = self.format_expr(else_expr_node, src);
                    else_clause.set_body(Body::new_body_with_expr(else_expr, self.state.depth));

                    cond_expr.set_else_clause(else_clause);
                }
                "END" => {
                    break;
                }
                "comment" => {
                    let comment_node = cursor.node();
                    let comment = Comment::new(comment_node, src);

                    // 行末コメントを式にセットする
                    cond_expr.set_trailing_comment(comment);
                }
                _ => unimplemented!(), // error
            }
        }

        self.unnest();
        self.unnest();

        cond_expr
    }

    fn format_between_and_expression(&mut self, node: Node, src: &str) -> AlignedExpr {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            panic!("between_and_expression has no children.");
        }

        // cursor -> expression
        let expr_node = cursor.node();
        let expr = self.format_expr(expr_node, src);

        goto_next_to_expr(&mut cursor);
        // cursor -> (NOT)? BETWEEN

        let mut operator = String::new();

        if cursor.node().kind() == "NOT" {
            operator += "NOT";
            operator += " "; // betweenの前に空白を入れる
            cursor.goto_next_sibling();
        }

        ensure_keyword(cursor.node(), "BETWEEN");
        operator += "BETWEEN";
        cursor.goto_next_sibling();
        // cursor -> expression

        let from_expr_node = cursor.node();
        let from_expr = self.format_expr(from_expr_node, src);
        goto_next_to_expr(&mut cursor);
        // cursor -> AND

        ensure_keyword(cursor.node(), "AND");
        cursor.goto_next_sibling();

        let to_expr_node = cursor.node();
        let to_expr = self.format_expr(to_expr_node, src);

        let mut rhs = AlignedExpr::new(from_expr, false);
        rhs.add_rhs("AND".to_string(), to_expr);

        let mut aligned = AlignedExpr::new(expr, false);
        aligned.add_rhs(operator, Expr::Aligned(Box::new(rhs)));

        aligned
    }
}

/// nodeが指定したキーワードノードかどうかをチェックする関数
/// 期待しているノードではない場合、panicする
fn ensure_keyword(node: Node, kw: &str) {
    if node.kind() != kw {
        panic!("excepted node is {}, but actual {}", kw, node.kind());
    }
}

/// _expressionの次のノードにcursorを移動させる関数
fn goto_next_to_expr(cursor: &mut TreeCursor) -> bool {
    if cursor.node().kind() == COMMENT {
        cursor.goto_next_sibling();
    }
    cursor.goto_next_sibling()
}
