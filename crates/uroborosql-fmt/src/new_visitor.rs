mod clause;
mod pg_expr;
mod statement;

use postgresql_cst_parser::syntax_kind::SyntaxKind;

pub(crate) const COMMA: &str = ",";

use crate::{
    config::CONFIG,
    cst::*,
    error::UroboroSQLFmtError,
    util::{convert_identifier_case, create_error_annotation},
};

pub(crate) struct Visitor {
    /// select文、insert文などが複数回出てきた際に1度だけSQL_IDを補完する、という処理を実現するためのフラグ
    should_complement_sql_id: bool,
}

impl Default for Visitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor {
    /// CONFIGファイルを見て、補完フラグがtrueの場合は`should_complement_sql_id`をtrueにして初期化する
    pub(crate) fn new() -> Visitor {
        Visitor {
            should_complement_sql_id: CONFIG.read().unwrap().complement_sql_id,
        }
    }

    /// sqlソースファイルをフォーマット用構造体に変形する
    pub(crate) fn visit_sql(
        &mut self,
        node: postgresql_cst_parser::tree_sitter::Node,
        src: &str,
    ) -> Result<Vec<Statement>, UroboroSQLFmtError> {
        // CSTを走査するTreeCursorを生成する
        // ほかの関数にはこのcursorの可変参照を渡す
        let mut cursor = node.walk();

        self.visit_source(&mut cursor, src)
    }

    /// source_file
    /// 呼び出し終了後、cursorはsource_fileを指している
    fn visit_source(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Vec<Statement>, UroboroSQLFmtError> {
        use postgresql_cst_parser::syntax_kind::SyntaxKind::{SelectStmt, Semicolon};

        // source_file -> _statement*
        let mut source: Vec<Statement> = vec![];

        if !cursor.goto_first_child() {
            // source_fileに子供がない、つまり、ソースファイルが空である場合
            // todo
            return Err(UroboroSQLFmtError::Unimplemented(
                // TODO: error_annotation_from_cursorの移植
                "visit_source(): source_file has no child\n".to_string(),
            ));
        }

        // ソースファイル先頭のコメントを保存するバッファ
        let mut comment_buf: Vec<Comment> = vec![];

        // 複数のStatement間のコメントの位置を決定するために使用する
        // 文を読んだが、対応するセミコロンを読んでいない場合はtrue、そうでない場合false
        let mut above_semi = true;

        loop {
            let kind = cursor.node().kind();

            match kind {
                stmt_kind @ SelectStmt
                //  | DeleteStmt | UpdateStmt | InsertStmt 
                => {
                    let mut stmt = match stmt_kind {
                        SelectStmt => self.visit_select_stmt(cursor, src)?, // とりあえず Statement を返す
                        // DeleteStmt => self.visit_delete_stmt(cursor, src)?,
                        // UpdateStmt => self.visit_update_stmt(cursor, src)?,
                        // InsertStmt => self.visit_insert_stmt(cursor, src)?,
                        _ => {
                            return Err(UroboroSQLFmtError::Unimplemented(
                                format!(
                                    "visit_source(): Unimplemented statement\n{}",
                                    pg_error_annotation_from_cursor(cursor, src)
                            )
                            ));
                        }
                    };

                    comment_buf
                        .iter()
                        .cloned()
                        .for_each(|c| stmt.add_comment(c));
                    comment_buf.clear();

                    source.push(stmt);
                    above_semi = true;
                }
                SyntaxKind::C_COMMENT| SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    if !source.is_empty() && above_semi {
                        let last_stmt = source.last_mut().unwrap();
                        // すでにstatementがある場合、末尾に追加
                        last_stmt.add_comment_to_child(comment)?;
                    } else {
                        // まだstatementがない場合、バッファに詰めておく
                        comment_buf.push(comment);
                    }
                }
                Semicolon => {
                    above_semi = false;
                    if let Some(last) = source.last_mut() {
                        last.set_semi(true);
                    }
                    // TODO: ; の上に文がない場合にどうなるか (tree-sitter-sqlでは構文エラーになる)
                }
                _ => {}
            };

            if !cursor.goto_next_sibling() {
                // 次の子供がいない場合、終了
                break;
            }
        }
        // cursorをsource_fileに戻す
        cursor.goto_parent();

        Ok(source)
    }

    /// カーソルが指すノードがSQL_IDであれば、clauseに追加する
    /// もし (_SQL_ID_が存在していない) && (_SQL_ID_がまだ出現していない) && (_SQL_ID_の補完がオン)
    /// の場合は補完する
    fn pg_consume_or_complement_sql_id(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        clause: &mut Clause,
    ) {
        if cursor.node().kind() == SyntaxKind::C_COMMENT {
            let text = cursor.node().text();

            if SqlID::is_sql_id(text) {
                clause.set_sql_id(SqlID::new(text.to_string()));
                cursor.goto_next_sibling();
                self.should_complement_sql_id = false;

                return;
            }
        }

        // SQL_IDがない、かつSQL補完フラグがtrueの場合、補完する
        if self.should_complement_sql_id {
            clause.set_sql_id(SqlID::new("/* _SQL_ID_ */".to_string()));
            self.should_complement_sql_id = false;
        }
    }

    /// カーソルが指すノードがコメントであれば、コメントを消費してclauseに追加する
    fn pg_consume_comments_in_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        clause: &mut Clause,
    ) -> Result<(), UroboroSQLFmtError> {
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            clause.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        Ok(())
    }
}

macro_rules! pg_ensure_kind {
    ($cursor:expr, expr: $keyword_expr:expr, $src:expr) => {{
        if $cursor.node().kind() != $keyword_expr {
            return Err($crate::UroboroSQLFmtError::UnexpectedSyntax(format!(
                "pg_ensure_kind!(): excepted node is {}, but actual {}\n{}",
                $keyword_expr,
                $cursor.node().kind(),
                $crate::new_visitor::pg_error_annotation_from_cursor($cursor, $src)
            )));
        }
    }};
    ($cursor:expr, $keyword_pattern:pat, $src:expr) => {
        if !matches!($cursor.node().kind(), $keyword_pattern) {
            return Err($crate::UroboroSQLFmtError::UnexpectedSyntax(format!(
                "pg_ensure_kind!(): excepted node is {}, but actual {}\n{}",
                stringify!($keyword_pattern),
                $cursor.node().kind(),
                $crate::new_visitor::pg_error_annotation_from_cursor($cursor, $src)
            )));
        }
    };
}

pub(crate) use pg_ensure_kind;

fn create_alias_from_expr(lhs: &Expr) -> Option<Expr> {
    let loc = lhs.loc();

    match lhs {
        Expr::Primary(prim) => prim.element().split('.').next_back().map(|last| {
            Expr::Primary(Box::new(PrimaryExpr::new(
                convert_identifier_case(last),
                loc,
            )))
        }),
        _ => None,
    }
}

macro_rules! pg_create_clause {
    ($cursor:expr, expr: $keyword_expr:expr) => {{
        pg_ensure_kind!($cursor, expr: $keyword_expr, $cursor.input);
        crate::new_visitor::Clause::from_pg_node($cursor.node())
    }};
    ($cursor:expr, $keyword_pattern:pat) => {{
        pg_ensure_kind!($cursor, $keyword_pattern, $cursor.input);
        crate::new_visitor::Clause::from_pg_node($cursor.node())
    }};
}

pub(crate) use pg_create_clause;

/// cursorからエラー注釈を作成する関数
/// 以下の形のエラー注釈を生成
///
/// ```sh
///   |
/// 2 | using tbl_b b
///   | ^^^^^^^^^^^^^ Appears as "ERROR" node on the CST
///   |
/// ```
fn pg_error_annotation_from_cursor(
    cursor: &postgresql_cst_parser::tree_sitter::TreeCursor,
    src: &str,
) -> String {
    let label = format!(r#"Appears as "{}" node on the CST"#, cursor.node().kind());
    let location = Location::from(cursor.node().range());

    match create_error_annotation(&location, &label, src) {
        Ok(error_annotation) => error_annotation,
        Err(_) => "".to_string(),
    }
}
