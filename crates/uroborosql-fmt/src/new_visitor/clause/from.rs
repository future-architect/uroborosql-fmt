use postgresql_cst_parser::syntax_kind::SyntaxKind;
use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind,
        expr::{ComplementConfig, ComplementKind},
        pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, Visitor,
    },
    util::convert_identifier_case,
};

impl Visitor {
    /// FROM句をClause構造体で返す
    pub(crate) fn visit_from_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clauseは必ずFROMを子供に持つ
        cursor.goto_first_child();

        // cursor -> FROM
        let mut clause = create_clause(cursor, src, "FROM")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> aliasable_expression
        // commaSep1(_aliasable_expression)

        // ASがあれば除去する
        // エイリアス補完は現状行わない
        let complement_config = ComplementConfig::new(ComplementKind::TableName, true, false);
        let body = self.visit_comma_sep_alias(cursor, src, Some(&complement_config))?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "from_clause", src)?;

        Ok(clause)
    }

    pub(crate) fn pg_visit_from_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clause = "FROM" from_list

        // cursor -> "FROM"
        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::FROM, src)?;

        let mut clause = pg_create_clause(cursor, SyntaxKind::FROM)?;
        cursor.goto_next_sibling();
        self.pg_consume_or_complement_sql_id(cursor, &mut clause);

        // cursor -> from_list
        pg_ensure_kind(cursor, SyntaxKind::from_list, src)?;

        let from_list = self.visit_from_list(cursor, src)?;

        clause.set_body(from_list);

        // cursor -> from_clause
        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::from_clause, src)?;

        Ok(clause)
    }

    /// postgresql-cst-parser の from_list を Body::SeparatedLines に変換する
    /// 呼出し後、cursor は from_list を指している
    pub(crate) fn visit_from_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // from_list -> table_ref ("," table_ref)*

        // from_listは必ず table_ref を子供に持つ
        // cursor -> table_ref
        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::table_ref, src)?;

        let mut sep_lines = SeparatedLines::new();

        // ASがあれば除去する
        // エイリアス補完は現状行わない
        let complement_config = ComplementConfig::new(ComplementKind::TableName, true, false);

        let table_ref = self.visit_table_ref(cursor, src, &complement_config)?;
        sep_lines.add_expr(table_ref, None, vec![]);

        while cursor.goto_next_sibling() {
            // cursor -> "," または table_ref
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::table_ref => {
                    let table_ref = self.visit_table_ref(cursor, src, &complement_config)?;
                    sep_lines.add_expr(table_ref, None, vec![]);
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_from_list(): unexpected node kind\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // cursor -> from_list
        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::from_list, src)?;

        Ok(Body::SepLines(sep_lines))
    }

    /// postgresql-cst-parser の table_ref を Body::SeparatedLines に変換する
    /// 呼出し後、cursor は table_ref を指している
    fn visit_table_ref(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
        complement_config: &ComplementConfig,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // table_ref
        // - relation_expr opt_alias_clause [tablesample_clause]
        // - select_with_parens opt_alias_clause
        // - joined_table
        // - '(' joined_table ')' alias_clause
        // - func_table func_alias_clause
        // - xmltable opt_alias_clause
        // - json_table opt_alias_clause
        // - LATERAL func_table func_alias_clause
        // - LATERAL xmltable opt_alias_clause
        // - LATERAL select_with_parens opt_alias_clause
        // - LATERAL json_table opt_alias_clause

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::relation_expr => {
                // テーブル参照
                // relation_expr opt_alias_clause [tablesample_clause]
                // - users as u
                // - schema1.table1 t1

                // AlignedExpr での左辺にあたる式
                let lhs_expr = self.visit_relation_expr(cursor, src)?;

                let aligned = AlignedExpr::new(lhs_expr);

                cursor.goto_next_sibling();

                if cursor.node().kind() == SyntaxKind::opt_alias_clause {
                    // TODO: エイリアスの追加
                    // TODO: 補完設定を参照
                    // let alias_clause = todo!();
                    // aligned.add_rhs(None, alias_clause);

                    cursor.goto_next_sibling();
                }

                if cursor.node().kind() == SyntaxKind::tablesample_clause {
                    // TABLESAMPLE
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_table_ref(): tablesample_clause node appeared. Tablesample is not implemented yet.\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }

                cursor.goto_parent();
                // cursor -> table_ref
                pg_ensure_kind(cursor, SyntaxKind::table_ref, src)?;

                Ok(aligned)
            }
            SyntaxKind::select_with_parens => {
                // サブクエリ
                // select_with_parens opt_alias_clause
                // - (SELECT * FROM users WHERE active = true) AS active_users
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): select_with_parens node appeared. Derived tables (subqueries) are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::joined_table => {
                // テーブル結合
                // joined_table
                // - users INNER JOIN orders ON users.id = orders.user_id
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): joined_table node appeared. Table joins are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::LParen => {
                // 括弧付き結合
                // '(' joined_table ')' alias_clause
                // - (users JOIN orders ON users.id = orders.user_id) AS uo
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): parenthesized join node appeared. Parenthesized joins are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::func_table => {
                // テーブル関数呼び出し
                // func_table func_alias_clause
                // - generate_series(1, 10) as g(val)
                // - unnest(array[1, 2, 3]) as nums(n)
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): func_table node appeared. Table function calls are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::LATERAL_P => {
                // LITERAL系
                // LATERAL func_table func_alias_clause
                // LATERAL xmltable opt_alias_clause
                // LATERAL select_with_parens opt_alias_clause
                // LATERAL json_table opt_alias_clause
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): LATERAL_P node appeared. LATERAL expressions are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::xmltable => {
                // XMLテーブル
                // xmltable opt_alias_clause
                // - XMLTABLE('/root/row' PASSING data COLUMNS id int PATH '@id')
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): xmltable node appeared. XML tables are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                // TODO: json_table
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_table_ref(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }
    }

    fn visit_relation_expr(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // relation_expr
        // - qualified_name
        // - extended_relation_expr

        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::qualified_name => self.visit_qualified_name(cursor, src)?,
            SyntaxKind::extended_relation_expr => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_relation_expr(): extended_relation_expr node appeared. Extended relation expressions are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_relation_expr(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::relation_expr, src)?;

        Ok(expr)
    }

    fn visit_qualified_name(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // qualified_name
        // - ColId
        // - ColId indirection

        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::ColId, src)?;

        let mut qualified_name_text = cursor.node().text().to_string();

        if cursor.goto_next_sibling() {
            pg_ensure_kind(cursor, SyntaxKind::indirection, src)?;
            // TODO: indirection 対応
            // この場所でのsubscriptは構文定義上可能だが、PostgreSQL側でrejectされる

            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_qualified_name(): indirection node appeared. Indirection is not implemented yet.\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        let primary = PrimaryExpr::new(
            convert_identifier_case(&qualified_name_text),
            cursor.node().range().into(),
        );

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::qualified_name, src)?;

        Ok(primary.into())
    }
}
