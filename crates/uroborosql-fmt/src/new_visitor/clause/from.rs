use postgresql_cst_parser::syntax_kind::SyntaxKind;
use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind, expr::{ComplementConfig, ComplementKind}, pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, Visitor
    },
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

        // ASがあれば除去する
        // エイリアス補完は現状行わない
        // let complement_config = ComplementConfig::new(ComplementKind::TableName, true, false);
        let from_list = self.visit_from_list(cursor, src)?;

        // for now, just return empty body
        let sep = SeparatedLines::new();
        let body = Body::SepLines(sep);

        clause.set_body(body);

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
        
        let table_ref = match cursor.node().kind() {
            SyntaxKind::relation_expr => {
                // テーブル参照
                // relation_expr opt_alias_clause [tablesample_clause]
                // - users as u
                // - schema1.table1 t1
                
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): relation_expr node appeared. Table references are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
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
        };
        
        
        let aligned = AlignedExpr::new(table_ref);

        Ok(aligned)
    }
}
