use super::*;
use postgresql_cst_parser::tree_sitter;

fn prepare_sql(sql: &str) -> Prepared {
    prepare(
        &tree_sitter::parse_2way(sql)
            .unwrap_or_else(|e| panic!("{sql}: {e:?}"))
            .root_node(),
    )
}

#[test]
fn supported_shapes_keep_requests_outputs_and_original_identifiers() {
    let sql = "SELECT u.\"a\"\"b\" AS \"別名\", +age, -age, (age + 1), age-1, age*2, age/2, 1.5, 'str', TRUE, FALSE, NULL FROM public.users AS u WHERE NOT (id < 18 OR name IS NULL) AND u.age >= 18 AND name IS NOT NULL;";
    let prepared = prepare_sql(sql);
    let select = prepared.statements[0].input.as_ref().expect("supported");
    assert_eq!(
        prepared.requests,
        vec![TableRequest {
            schema: Some("public".into()),
            name: "users".into()
        }]
    );
    assert_eq!(select.targets.len(), 12);
    assert_eq!(select.targets[0].alias.as_ref().unwrap().name, "別名");
    let Expr::Column(reference) = &select.targets[0].expr else {
        panic!()
    };
    assert_eq!(reference.column.name, "a\"b");
    assert!(reference.column.quoted);
    assert_eq!(reference.column.spelling, "\"a\"\"b\"");
    assert_eq!(
        &sql[reference.column.range.start_byte..reference.column.range.end_byte],
        "\"a\"\"b\""
    );
    assert_eq!(reference.qualifier.as_ref().unwrap().name, "u");
}

#[test]
fn accepts_each_operator_and_identifier_keyword() {
    for expression in [
        "id",
        "name",
        "u.id",
        "1",
        "'s'",
        "1.5",
        "TRUE",
        "FALSE",
        "NULL",
        "+id",
        "-id",
        "id+1",
        "id-1",
        "id*1",
        "id/1",
        "id=1",
        "id<>1",
        "id!=1",
        "id<1",
        "id<=1",
        "id>1",
        "id>=1",
        "TRUE AND FALSE",
        "TRUE OR FALSE",
        "NOT TRUE",
        "id IS NULL",
        "id IS NOT NULL",
        "(id)",
    ] {
        let sql = format!("SELECT {expression} FROM USERS u;");
        let input = prepare_sql(&sql);
        assert!(input.statements[0].input.is_ok(), "{sql}: {input:?}");
        assert_eq!(input.requests[0].name, "users");
    }
}

#[test]
fn rejects_unlisted_syntax_and_recovered_shapes_before_collecting_requests() {
    for sql in [
        "SELECT * FROM users",
        "SELECT u.* FROM users u",
        "SELECT id alias FROM users",
        "SELECT DISTINCT id FROM users",
        "SELECT id FROM users ORDER BY id",
        "SELECT id FROM users GROUP BY id",
        "SELECT id FROM users HAVING TRUE",
        "SELECT id FROM users LIMIT 1",
        "SELECT id FROM users OFFSET 1",
        "SELECT id INTO other FROM users",
        "SELECT ALL id FROM users",
        "SELECT id FROM ONLY users",
        "SELECT id FROM users *",
        "SELECT id FROM users u(a)",
        "SELECT id FROM users, others",
        "SELECT id FROM users JOIN others ON TRUE",
        "SELECT id FROM (SELECT id FROM users) s",
        "WITH t AS (SELECT id FROM users) SELECT id FROM t",
        "SELECT id FROM users UNION SELECT id FROM users",
        "SELECT id FROM users INTERSECT SELECT id FROM users",
        "SELECT id FROM users EXCEPT SELECT id FROM users",
        "UPDATE users SET id = 1",
        "SELECT count(*) FROM users",
        "SELECT id::text FROM users",
        "SELECT CAST(id AS text) FROM users",
        "SELECT CASE WHEN TRUE THEN id END FROM users",
        "SELECT id IN (1) FROM users",
        "SELECT id BETWEEN 1 AND 2 FROM users",
        "SELECT name LIKE 'a' FROM users",
        "SELECT row_number() OVER () FROM users",
        "SELECT public.users.id FROM users",
        "SELECT id[1] FROM users",
        "SELECT id % 2 FROM users",
        "SELECT id IS TRUE FROM users",
        "SELECT id COLLATE \"C\" FROM users",
        "SELECT , id FROM users",
        "SELECT id FROM users WHERE AND id = 1",
        "SELECT id FROM users TABLESAMPLE SYSTEM(1)",
    ] {
        let input = prepare_sql(sql);
        assert!(input.requests.is_empty(), "{sql}: {input:?}");
        assert!(input.statements.iter().all(|s| s.input.is_err()), "{sql}");
    }
}

#[test]
fn session_changes_exclude_the_whole_file_but_not_other_unsupported_statements() {
    for change in [
        "SET search_path = public",
        "RESET search_path",
        "RESET ALL",
        "SET ROLE NONE",
        "SET SESSION AUTHORIZATION DEFAULT",
        "SET SCHEMA 'public'",
        "SET LOCAL work_mem = '4MB'",
        "SET SESSION work_mem = DEFAULT",
        "DISCARD ALL",
        "DISCARD PLANS",
    ] {
        for sql in [
            format!("{change}; SELECT id FROM users"),
            format!("SELECT id FROM users; {change}; RESET ALL; SELECT id FROM users"),
        ] {
            let input = prepare_sql(&sql);
            assert!(input.requests.is_empty(), "{sql}");
            assert!(
                input
                    .statements
                    .iter()
                    .all(|s| matches!(s.input, Err(Exclusion::SessionChange))),
                "{sql}"
            );
        }
    }
    let input = prepare_sql("SET CONSTRAINTS ALL DEFERRED; SELECT id FROM users;");
    assert_eq!(input.requests.len(), 1);
    assert!(input.statements[1].input.is_ok());
}

#[test]
fn ordinary_comments_strings_and_mixed_statements_do_not_hide_eligible_sql() {
    let input = prepare_sql("-- SET ROLE\nSELECT /* ordinary comment */ 'SET ROLE' AS label, name FROM users; SELECT count(*) FROM users; SELECT id FROM users;");
    assert_eq!(
        input
            .statements
            .iter()
            .map(|s| s.input.is_ok())
            .collect::<Vec<_>>(),
        [true, false, true]
    );
    assert_eq!(input.requests.len(), 1);
}

#[test]
fn directive_blocks_cross_statements_and_end_before_following_statement() {
    for sql in ["/*IF cond*/ SELECT id FROM users; SELECT name FROM users; /*END*/ SELECT id FROM users;", "/*BEGIN*/ /*IF cond*/ SELECT id FROM users; /*ELSE*/ SELECT name FROM users; /*END*/ /*END*/ SELECT id FROM users;", "/*%if cond*/ SELECT id FROM users; /*%elseif other*/ SELECT name FROM users; /*%end*/ SELECT id FROM users;"] {
        let input = prepare_sql(sql);
        assert_eq!(input.statements.iter().map(|s| s.input.is_ok()).collect::<Vec<_>>(), [false,false,true], "{sql}");
    }
    let input =
        prepare_sql("SELECT id FROM users WHERE /*IF cond*/ id=1 /*END*/; SELECT id FROM users;");
    assert_eq!(
        input
            .statements
            .iter()
            .map(|s| s.input.is_ok())
            .collect::<Vec<_>>(),
        [false, true]
    );
}

#[test]
fn malformed_directives_and_dynamic_sql_are_conservative() {
    for sql in ["/*IF cond*/ SELECT id FROM users; SELECT id FROM users;", "SELECT id FROM users; /*END*/ SELECT id FROM users", "/*BEGIN*/ SELECT id FROM users; /*ELSE*/ SELECT id FROM users; /*END*/", "/*IF cond*/ SELECT id FROM users; /*ELSE*/ SELECT id FROM users; /*ELIF other*/ SELECT id FROM users; /*END*/", "SELECT /*id*/1 FROM users", "SELECT id FROM /*#table*/users", "SELECT /*$column*/id FROM users", "SELECT /*id*/ ;", "SELECT id FROM /*#table*/ ;"] {
        let input = prepare_sql(sql);
        assert!(input.requests.is_empty(), "{sql}: {input:?}");
    }
}

#[test]
fn temp_alias_and_uncertain_identifier_normalization_are_excluded() {
    for schema in ["pg_temp", "PG_TEMP", "\"pg_temp\""] {
        assert!(matches!(
            prepare_sql(&format!("SELECT id FROM {schema}.users")).statements[0].input,
            Err(Exclusion::TemporarySchema)
        ));
    }
    for schema in ["\"PG_TEMP\"", "pg_temp_data"] {
        assert!(
            prepare_sql(&format!("SELECT id FROM {schema}.users")).statements[0]
                .input
                .is_ok()
        );
    }
    for name in ["a".repeat(64), format!("\"{}\"", "あ".repeat(22))] {
        let input = prepare_sql(&format!("SELECT {name} FROM users"));
        assert!(input.requests.is_empty(), "{name}");
    }
}

#[test]
fn preparation_owns_data_after_tree_drops() {
    let prepared = prepare_sql("SELECT id, id FROM users; SELECT name FROM users");
    assert_eq!(prepared.requests.len(), 1);
    assert_eq!(prepared.statements.len(), 2);
    assert_eq!(
        prepared.statements[0].input.as_ref().unwrap().targets.len(),
        2
    );
}

#[test]
fn unrecoverable_shapes_preserve_parser_errors() {
    for sql in [
        "SELECT id FROM users WHERE id = 1 AND",
        "SELECT U&\"d\\0061t\" FROM users",
        "SELECT FROM ;",
        "SELECT id, FROM users;",
    ] {
        assert!(tree_sitter::parse_2way(sql).is_err(), "{sql}");
    }
}

#[test]
fn bind_comments_accept_signed_parenthesized_and_nonliteral_samples() {
    for sample in ["-1", "+1", "(1)", "TRUE", "id", "'text'", " 1"] {
        let sql =
            format!("SELECT nmae FROM users WHERE id = /*id*/{sample}; SELECT id FROM users;");
        let prepared = prepare_sql(&sql);
        assert!(
            matches!(prepared.statements[0].input, Err(Exclusion::TwoWaySql)),
            "{sql}"
        );
        assert!(prepared.statements[1].input.is_ok(), "{sql}");
    }
    assert!(matches!(
        prepare_sql("SELECT nmae FROM users /*param*/;").statements[0].input,
        Err(Exclusion::TwoWaySql)
    ));
}

#[test]
fn ordinary_comment_before_sample_like_expression_is_not_a_bind() {
    for comment in [
        "/* ordinary comment */",
        "/*\tordinary */",
        "/*+ hint */",
        "/*123 ordinary */",
    ] {
        for sample in ["1", "-1", "(1)"] {
            let sql = format!("SELECT nmae FROM users WHERE id = {comment}{sample}");
            assert!(prepare_sql(&sql).statements[0].input.is_ok(), "{sql}");
        }
    }
}
