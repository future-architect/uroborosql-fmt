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
        // CSTを走査するTreeCursorを生成する
        // ほかの関数にはこのcursorの可変参照を渡す
        let mut cursor = node.walk();

        self.format_source(&mut cursor, src)
    }

    // ネストを1つ深くする
    fn nest(&mut self) {
        self.state.depth += 1;
    }

    // ネストを1つ浅くする
    fn unnest(&mut self) {
        self.state.depth -= 1;
    }

    /// source_file
    /// 呼び出し終了後、cursorはsource_fileを指している
    fn format_source(&mut self, cursor: &mut TreeCursor, src: &str) -> Vec<Statement> {
        // source_file -> _statement*
        let mut source: Vec<Statement> = vec![];

        if !cursor.goto_first_child() {
            // source_fileに子供がない、つまり、ソースファイルが空である場合
            todo!()
        }

        // ソースファイル先頭のコメントを保存するバッファ
        let mut comment_buf: Vec<Comment> = vec![];

        loop {
            match cursor.node().kind() {
                "select_statement" => {
                    let mut stmt = self.format_select_stmt(cursor, src);

                    // コメントが以前にあれば先頭に追加
                    comment_buf
                        .iter()
                        .cloned()
                        .for_each(|c| stmt.add_comment(c));
                    comment_buf.clear();

                    source.push(stmt);
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);

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
        // cursorをsource_fileに戻す
        cursor.goto_parent();

        source
    }

    /// SELECT文
    /// 呼び出し後、cursorはselect_statementを指す
    fn format_select_stmt(&mut self, cursor: &mut TreeCursor, src: &str) -> Statement {
        /*
            _select_statement ->
                select_clause
                from_clause?
                where_clause?
        */

        let mut statement = Statement::new(self.state.depth);

        // select_statementは必ずselect_clauseを子供に持つ
        cursor.goto_first_child();

        // cursor -> select_clause

        // select句を追加する
        statement.add_clause(self.format_select_clause(cursor, src));

        // from句以下を追加する
        while cursor.goto_next_sibling() {
            // 次の兄弟へ移動
            // select_statementの子供がいなくなったら終了
            match cursor.node().kind() {
                "from_clause" => {
                    // from_clauseは必ずFROMを子供に持つ
                    cursor.goto_first_child();

                    // cursor -> FROM
                    ensure_kind(cursor, "FROM");
                    let mut clause = Clause::new(
                        "FROM".to_string(),
                        Location::new(cursor.node().range()),
                        self.state.depth,
                    );
                    let mut separated_lines = SeparatedLines::new(self.state.depth, ",", true);

                    cursor.goto_next_sibling();
                    // cursor -> comments | _aliasable_expression

                    while cursor.node().kind() == COMMENT {
                        clause.add_comment_to_child(Comment::new(cursor.node(), src));
                        cursor.goto_next_sibling();
                    }

                    // cursor -> aliasable_expression
                    let alias = self.format_aliasable_expr(cursor, src);
                    separated_lines.add_expr(alias);

                    // ("," _aliasable_expression)*
                    while cursor.goto_next_sibling() {
                        // cursor -> , または comment または _aliasable_expression
                        match cursor.node().kind() {
                            "," => continue,
                            COMMENT => {
                                separated_lines
                                    .add_comment_to_child(Comment::new(cursor.node(), src));
                            }
                            _ => {
                                // _aliasable_expression
                                let alias = self.format_aliasable_expr(cursor, src);
                                separated_lines.add_expr(alias);
                            }
                        }
                    }

                    clause.set_body(Body::SepLines(separated_lines));
                    statement.add_clause(clause);

                    // cursorをfrom_clauseに戻す
                    cursor.goto_parent();
                    ensure_kind(cursor, "from_clause");
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    // where_clauseは必ずWHEREを子供に持つ
                    cursor.goto_first_child();

                    // cursor -> WHERE
                    ensure_kind(cursor, "WHERE");

                    let where_node = cursor.node();
                    let mut clause = Clause::new(
                        "WHERE".to_string(),
                        Location::new(where_node.range()),
                        self.state.depth,
                    );

                    cursor.goto_next_sibling();
                    // cursor -> COMMENT | _expression

                    // TODO: コメントノードを消費する処理を関数かメソッドにまとめる
                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        clause.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // cursor -> _expression
                    let expr = self.format_expr(cursor, src);

                    // 結果として得られた式をBodyに変換する
                    // TODO: 関数またはBody、Exprのメソッドとして定義する
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

                    clause.set_body(body);
                    statement.add_clause(clause);

                    // cursorをwhere_clauseに戻す
                    cursor.goto_parent();
                    ensure_kind(cursor, "where_clause");
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
                    let select_stmt = self.format_select_stmt(cursor, src);
                    select_stmt
                        .get_clauses()
                        .iter()
                        .for_each(|clause| statement.add_clause(clause.to_owned()));

                    // cursorはselect_statementになっているはずである
                }
                COMMENT => statement.add_comment_to_child(Comment::new(cursor.node(), src)),
                _ => {
                    break;
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "select_statement");

        statement
    }

    /// SELECT句
    /// 呼び出し後、cursorはselect_clauseを指している
    fn format_select_clause(&mut self, cursor: &mut TreeCursor, src: &str) -> Clause {
        /*
            select_clause ->
                "SELECT"
                select_clause_body
        */

        // select_clauseは必ずSELECTを子供に持っているはずである
        cursor.goto_first_child();

        // cursor -> SELECT
        ensure_kind(cursor, "SELECT");
        let mut clause = Clause::new(
            "SELECT".to_string(),
            Location::new(cursor.node().range()),
            self.state.depth,
        );

        cursor.goto_next_sibling();
        // cursor -> comments | select_clause_body

        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);

            // _SQL_ID_かどうかをチェックする
            if comment.is_sql_id_comment() {
                clause.set_sql_id(comment);
            } else {
                clause.add_comment_to_child(comment)
            }

            cursor.goto_next_sibling();
        }
        // cursor -> select_caluse_body

        let body = self.format_select_clause_body(cursor, src);
        clause.set_body(Body::SepLines(body));

        // cursorをselect_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause");

        clause
    }

    /// SELECT句の本体をSeparatedLinesで返す
    /// 呼び出し後、cursorはselect_clause_bodyを指している
    fn format_select_clause_body(&mut self, cursor: &mut TreeCursor, src: &str) -> SeparatedLines {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        // select_clause_bodyは必ず_aliasable_expressionを子供に持つ
        cursor.goto_first_child();

        // cursor -> _aliasable_expression

        let mut separated_lines = SeparatedLines::new(self.state.depth, ",", false);

        let aligned = self.format_aliasable_expr(cursor, src);
        separated_lines.add_expr(aligned);

        // (',' _aliasable_expression)*
        while cursor.goto_next_sibling() {
            // cursor -> , または COMMENT または _aliasable_expression
            let child_node = cursor.node();
            match child_node.kind() {
                "," => continue,
                COMMENT => {
                    separated_lines.add_comment_to_child(Comment::new(child_node, src));
                }
                _ => {
                    // _aliasable_expression
                    let aligned = self.format_aliasable_expr(cursor, src);
                    separated_lines.add_expr(aligned);
                }
            }
        }

        // cursorをselect_clause_bodyに
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause_body");

        separated_lines
    }

    /// エイリアス可能な式
    /// 呼び出し後、cursorはaliasまたは式のノードを指している
    fn format_aliasable_expr(&mut self, cursor: &mut TreeCursor, src: &str) -> AlignedExpr {
        /*
            _aliasable_expression ->
                alias | _expression

            alias ->
                _expression
                "AS"?
                identifier
                << 未対応!! "(" identifier ("," identifier)* ")" >>
        */
        match cursor.node().kind() {
            "alias" => {
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let lhs_expr = self.format_expr(cursor, src);

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

                // cursorをalias に戻す
                cursor.goto_parent();

                aligned
            }
            _ => {
                // _expression
                let expr = self.format_expr(cursor, src);

                AlignedExpr::new(expr, true)
            }
        }
    }

    /// 引数の文字列が比較演算子かどうかを判定する
    fn is_comp_op(op_str: &str) -> bool {
        matches!(
            op_str,
            "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
        )
    }

    /// 式のフォーマットを行う。
    /// cursorがコメントを指している場合、バインドパラメータであれば結合して返す。
    /// 式の初めにバインドパラメータが現れた場合、式の本体は隣の兄弟ノードになる。
    /// 呼び出し後、cursorは式の本体のノードを指す
    fn format_expr(&mut self, cursor: &mut TreeCursor, src: &str) -> Expr {
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
                ensure_kind(cursor, "dotted_name");

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => {
                // cursor -> binary_expression

                cursor.goto_first_child();
                // cursor -> _expression

                // 左辺
                let lhs_expr = self.format_expr(cursor, src);

                cursor.goto_next_sibling();
                // cursor -> op (e.g., "+", "-", "=", ...)

                // 演算子
                let op_node = cursor.node();
                let op_str = op_node.utf8_text(src.as_ref()).unwrap();

                cursor.goto_next_sibling();
                // cursor -> _expression

                // 右辺
                let rhs_expr = self.format_expr(cursor, src);

                // cursorを戻しておく
                cursor.goto_parent();
                ensure_kind(cursor, "binary_expression");

                if Self::is_comp_op(op_str) {
                    // 比較演算子ならばそろえる必要があるため、AlignedExprとする
                    let mut aligned = AlignedExpr::new(lhs_expr, false);
                    aligned.add_rhs(op_str.to_string(), rhs_expr);

                    Expr::Aligned(Box::new(aligned))
                } else {
                    // 比較演算子でないならば、PrimaryExprに
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
                Expr::Aligned(Box::new(self.format_between_and_expression(cursor, src)))
            }
            "boolean_expression" => self.format_bool_expr(cursor, src),
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let primary = PrimaryExpr::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(cursor.node().range()),
                );

                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                self.nest();
                let select_subexpr = self.format_select_subexpr(cursor, src);
                self.unnest();
                Expr::SelectSub(Box::new(select_subexpr))
            }
            "parenthesized_expression" => {
                let paren_expr = self.format_paren_expr(cursor, src);
                Expr::ParenExpr(Box::new(paren_expr))
            }
            "asterisk_expression" => {
                let asterisk = AsteriskExpr::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(cursor.node().range()),
                );
                Expr::Asterisk(Box::new(asterisk))
            }
            "conditional_expression" => {
                let cond_expr = self.format_cond_expr(cursor, src);
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
        };

        // バインドパラメータの追加
        if let Some(comment) = head_comment {
            if comment.is_multi_line_comment() && comment.loc().is_next_to(&result.loc()) {
                // 複数行コメントかつ式に隣接していれば、バインドパラメータ
                result.set_head_comment(comment);
            } else {
                // TODO: 隣接していないコメント
                todo!()
            }
        }

        result
    }

    /// bool式をフォーマットする
    /// 呼び出し後、cursorはboolean_expressionを指している
    fn format_bool_expr(&mut self, cursor: &mut TreeCursor, src: &str) -> Expr {
        /*
        boolean_expression: $ =>
            choice(
            prec.left(PREC.unary, seq(kw("NOT"), $._expression)),
            prec.left(PREC.and, seq($._expression, kw("AND"), $._expression)),
            prec.left(PREC.or, seq($._expression, kw("OR"), $._expression)),
        ),
         */

        let mut boolean_expr = BooleanExpr::new(self.state.depth, "-");

        cursor.goto_first_child();

        if cursor.node().kind() == "NOT" {
            // TODO: NOT
            todo!();
        } else {
            // and or
            let left = self.format_expr(cursor, src);

            // CST上ではbool式は(left op right)のような構造になっている
            // BooleanExprでは(expr1 op expr2 ... exprn)のようにフラットに保持するため、左辺がbool式ならmergeメソッドでマージする
            // また、要素をAlignedExprで保持するため、AlignedExprでない場合ラップする
            // TODO: BooleanExprかExprのメソッド、または関数として定義したほうがよい可能性がある
            match left {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::ParenExpr(paren_expr) => {
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), false);
                    boolean_expr.add_expr(aligned);
                }
                _ => todo!(),
            }

            cursor.goto_next_sibling();
            // cursor -> COMMENT | op

            while cursor.node().kind() == COMMENT {
                boolean_expr.add_comment_to_child(Comment::new(cursor.node(), src));
                cursor.goto_next_sibling();
            }

            let sep = cursor.node().kind();
            boolean_expr.set_default_separator(sep.to_string());

            cursor.goto_next_sibling();
            // cursor -> _expression

            let right = self.format_expr(cursor, src);

            // 左辺と同様の処理を行う
            match right {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::ParenExpr(paren_expr) => {
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), false);
                    boolean_expr.add_expr(aligned);
                }
                _ => todo!(),
            }
        }
        // cursorをboolean_expressionに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "boolean_expression");

        Expr::Boolean(Box::new(boolean_expr))
    }

    /// かっこで囲まれたSELECTサブクエリをフォーマットする
    /// 呼び出し後、cursorはselect_subexpressionを指している
    fn format_select_subexpr(&mut self, cursor: &mut TreeCursor, src: &str) -> SelectSubExpr {
        // select_subexpression -> "(" select_statement ")"

        let loc = Location::new(cursor.node().range());

        // cursor -> select_subexpression

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
        let mut select_stmt = self.format_select_stmt(cursor, src);

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

        cursor.goto_parent();
        ensure_kind(cursor, "select_subexpression");

        SelectSubExpr::new(select_stmt, loc, self.state.depth)
    }

    /// かっこで囲まれた式をフォーマットする
    /// 呼び出し後、cursorはparenthesized_expressionを指す
    fn format_paren_expr(&mut self, cursor: &mut TreeCursor, src: &str) -> ParenExpr {
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

        let expr = self.format_expr(cursor, src);

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
            paren_expr.add_comment_to_child(Comment::new(cursor.node(), src));
            cursor.goto_next_sibling();
        }

        // tree-sitter-sqlを修正したら削除する
        cursor.goto_parent();
        ensure_kind(cursor, "parenthesized_expression");

        paren_expr
    }

    /// CASE式をフォーマットする
    /// 呼び出し後、cursorはconditional_expressionを指す
    fn format_cond_expr(&mut self, cursor: &mut TreeCursor, src: &str) -> CondExpr {
        // conditional_expression ->
        //     "CASE"
        //     ("WHEN" expression "THEN" expression)*
        //     ("ELSE" expression)?
        //     "END"

        let mut cond_expr = CondExpr::new(Location::new(cursor.node().range()), self.state.depth);

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

                    let when_expr = self.format_expr(cursor, src);
                    when_clause.set_body(Body::new_body_with_expr(when_expr, self.state.depth));

                    cursor.goto_next_sibling();
                    // cursor -> comment || "THEN"

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        when_clause.add_comment_to_child(comment);
                        cursor.goto_next_sibling();
                    }

                    // cursor -> "THEN"
                    ensure_kind(cursor, "THEN");
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

                    let then_expr = self.format_expr(cursor, src);
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

                    let else_expr = self.format_expr(cursor, src);
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

        cursor.goto_parent();
        ensure_kind(cursor, "conditional_expression");

        cond_expr
    }

    /// BETWEEN述語をフォーマットする
    /// 呼び出し後、cursorはbetween_and_expressionを指す
    fn format_between_and_expression(&mut self, cursor: &mut TreeCursor, src: &str) -> AlignedExpr {
        // between_and_expressionに子供がいないことはない
        cursor.goto_first_child();
        // cursor -> expression

        let expr = self.format_expr(cursor, src);

        cursor.goto_next_sibling();
        // cursor -> (NOT)? BETWEEN

        let mut operator = String::new();

        if cursor.node().kind() == "NOT" {
            operator += "NOT";
            operator += " "; // betweenの前に空白を入れる
            cursor.goto_next_sibling();
        }

        ensure_kind(cursor, "BETWEEN");
        operator += "BETWEEN";
        cursor.goto_next_sibling();
        // cursor -> _expression

        let from_expr = self.format_expr(cursor, src);
        cursor.goto_next_sibling();
        // cursor -> AND

        ensure_kind(cursor, "AND");
        cursor.goto_next_sibling();
        // cursor -> _expression

        let to_expr = self.format_expr(cursor, src);

        // (from AND to)をAlignedExprにまとめる
        let mut rhs = AlignedExpr::new(from_expr, false);
        rhs.add_rhs("AND".to_string(), to_expr);

        // (expr BETWEEN rhs)をAlignedExprにまとめる
        let mut aligned = AlignedExpr::new(expr, false);
        aligned.add_rhs(operator, Expr::Aligned(Box::new(rhs)));

        cursor.goto_parent();
        ensure_kind(cursor, "between_and_expression");

        aligned
    }
}

/// cursorが指定した種類のノードを指しているかどうかをチェックする関数
/// 期待しているノードではない場合、panicする
fn ensure_kind(cursor: &TreeCursor, kind: &str) {
    if cursor.node().kind() != kind {
        panic!(
            "excepted node is {}, but actual {}",
            kind,
            cursor.node().kind()
        );
    }
}
