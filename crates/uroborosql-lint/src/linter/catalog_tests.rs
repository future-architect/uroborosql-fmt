use super::*;
use crate::{
    catalog::{
        AbsenceKind, AcquisitionErrorKind, AcquisitionPhase, CatalogEntry, CatalogProvider,
        ColumnDefinition, InMemoryCatalogProvider, Lookup, TableDefinition, UnknownReason,
    },
    diagnostic::Severity,
    resolution::{
        self, AnalysisStatus, Catalog, Exclusion, Referent, Resolution, ResolutionUnknown,
    },
    rules::{NoDistinct, NoUnknownReference, RuleEnum},
    ConfigStore,
};

fn config() -> ResolvedLintConfig {
    ResolvedLintConfig {
        rules: vec![(
            RuleEnum::NoUnknownReference(NoUnknownReference),
            Severity::Error,
        )],
        db: None,
    }
}
fn table(schema: &str, name: &str, columns: &[&str]) -> TableDefinition {
    TableDefinition {
        schema: schema.into(),
        name: name.into(),
        columns: columns
            .iter()
            .map(|s| ColumnDefinition { name: (*s).into() })
            .collect(),
        system_columns: vec![ColumnDefinition {
            name: "ctid".into(),
        }],
    }
}
fn snapshot() -> CatalogSnapshot {
    CatalogSnapshot::new(
        vec!["public".into()],
        [
            CatalogEntry {
                schema: "public".into(),
                table: "users".into(),
                outcome: Lookup::Found(table("public", "users", &["id", "name", "age"])),
            },
            CatalogEntry {
                schema: "public".into(),
                table: "usres".into(),
                outcome: Lookup::Absent(AbsenceKind::Table),
            },
            CatalogEntry {
                schema: "public".into(),
                table: "orders".into(),
                outcome: Lookup::Found(table("public", "orders", &["id", "user_id", "amount"])),
            },
        ],
    )
    .unwrap()
}
fn run(sql: &str) -> CatalogLintResult {
    Linter::new()
        .run_with_catalog(sql, &config(), Ok(&snapshot()))
        .unwrap()
}
fn slices<'a>(sql: &'a str, diagnostics: &[Diagnostic]) -> Vec<&'a str> {
    diagnostics
        .iter()
        .map(|d| &sql[d.span.start.byte..d.span.end.byte])
        .collect()
}

#[test]
fn wildcard_targets_diagnose_visible_names_and_other_references() {
    for (sql, expected) in [
        ("SELECT * FROM users", vec![]),
        ("SELECT u.* FROM users u", vec![]),
        ("SELECT users.* FROM users u", vec!["users"]),
        ("SELECT x.* FROM users u", vec!["x"]),
        ("SELECT \"U\".* FROM users \"U\"", vec![]),
        ("SELECT u.* FROM users \"U\"", vec!["u"]),
        (
            "SELECT x.*, missing, u.* FROM users u WHERE absent = 1",
            vec!["x", "missing", "absent"],
        ),
        ("SELECT * FROM usres", vec!["usres"]),
        (
            "SELECT users.* FROM usres u WHERE absent = 1",
            vec!["usres"],
        ),
        ("SELECT id * 2 FROM users", vec![]),
        ("SELECT DISTINCT * FROM users LIMIT ALL", vec![]),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
    }
    let sql = "SELECT public.users.* FROM users; SELECT x.*, missing FROM users u WHERE absent = 1";
    let result = run(sql);
    assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    assert_eq!(slices(sql, &result.diagnostics), ["x", "missing", "absent"]);
}

#[test]
fn joins_and_grouping_clauses_resolve_against_every_from_item() {
    for (sql, expected) in [
        (
            "SELECT u.name, o.amount FROM users u JOIN orders o ON o.user_id = u.id",
            vec![],
        ),
        (
            "SELECT u.nmae, o.total FROM users u JOIN orders o ON o.uid = u.id",
            vec!["nmae", "total", "uid"],
        ),
        ("SELECT name, amount FROM users, orders", vec![]),
        ("SELECT name, x.amount FROM users, orders o", vec!["x"]),
        (
            "SELECT name FROM users u LEFT JOIN orders o USING (id) WHERE agge > 1",
            vec!["agge"],
        ),
        ("SELECT name FROM users JOIN usres ON id = 1", vec!["usres"]),
        ("SELECT name FROM users ORDER BY nmae", vec!["nmae"]),
        ("SELECT name FROM users ORDER BY age DESC, 1", vec![]),
        ("SELECT id AS user_id FROM users ORDER BY user_id", vec![]),
        (
            "SELECT id AS user_id FROM users ORDER BY u.user_id",
            vec!["u"],
        ),
        (
            "SELECT age, count FROM users GROUP BY age, agge HAVING max_age > 1",
            vec!["count", "agge", "max_age"],
        ),
        ("SELECT age AS a FROM users GROUP BY a", vec![]),
        (
            "SELECT age AS a FROM users WHERE a > 1 HAVING a > 1",
            vec!["a", "a"],
        ),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert_eq!(
            result.statements[0].status,
            AnalysisStatus::Complete,
            "{sql}"
        );
    }
}

#[test]
fn subqueries_see_enclosing_from_items_and_hide_their_own_columns() {
    for (sql, expected) in [
        (
            "SELECT id FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id)",
            vec![],
        ),
        (
            "SELECT id FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.uid = u.idd)",
            vec!["uid", "idd"],
        ),
        (
            "SELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE name <> '')",
            vec![],
        ),
        (
            "SELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE nmae <> '')",
            vec!["nmae"],
        ),
        (
            "SELECT s.id, s.anything FROM (SELECT id FROM users) AS s",
            vec![],
        ),
        ("SELECT anything FROM (SELECT id FROM users) AS s", vec![]),
        ("SELECT x.id FROM (SELECT id FROM users) AS s", vec![]),
        (
            "SELECT s.id FROM (SELECT nmae FROM users) AS s",
            vec!["nmae"],
        ),
        (
            "SELECT name FROM users u, (SELECT 1 AS one) s WHERE agge > one",
            vec![],
        ),
        // Subqueries in JOIN ON see every FROM item of the query.
        (
            "SELECT u.name FROM users u JOIN orders o ON o.user_id IN (SELECT o2.user_id FROM orders o2 WHERE o2.amount > u.age AND amount > age)",
            vec![],
        ),
        (
            "SELECT 1 FROM users u JOIN orders o ON EXISTS (SELECT 1 FROM orders o2 WHERE o2.id = o.idd)",
            vec!["idd"],
        ),
        // A CTE inside a subquery can correlate to the enclosing query.
        (
            "SELECT 1 FROM users u WHERE EXISTS (WITH c AS (SELECT u.id AS x) SELECT 1 FROM c)",
            vec![],
        ),
        // Parenthesized set-operation arms are checked on their own.
        (
            "(SELECT nmae FROM users) UNION (SELECT id FROM orders) ORDER BY 1",
            vec!["nmae"],
        ),
        ("(SELECT nmae FROM users) ORDER BY 1", vec!["nmae"]),
        (
            "SELECT id AS aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, name AS n FROM users ORDER BY n",
            vec![],
        ),
        // A derived table cannot see its sibling FROM items.
        ("SELECT 1 FROM users u, (SELECT u.id) s", vec!["u"]),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert_eq!(
            result.statements[0].status,
            AnalysisStatus::Complete,
            "{sql}"
        );
    }
}

#[test]
fn ctes_and_set_operations_never_borrow_another_scope() {
    for (sql, expected) in [
        (
            "WITH c AS (SELECT id FROM users) SELECT id, other FROM c",
            vec![],
        ),
        (
            "WITH users AS (SELECT 1 AS one) SELECT one FROM users",
            vec![],
        ),
        (
            "WITH c AS (SELECT nmae FROM users) SELECT * FROM c",
            vec!["nmae"],
        ),
        // A CTE body cannot see the FROM items of the statement that defines it.
        (
            "WITH c AS (SELECT u.id) SELECT * FROM c, users u",
            vec!["u"],
        ),
        (
            "SELECT id FROM users UNION SELECT amount FROM orders",
            vec![],
        ),
        (
            "SELECT name FROM users UNION ALL SELECT name FROM orders",
            vec!["name"],
        ),
        (
            "SELECT id FROM users EXCEPT SELECT user_id FROM orders ORDER BY anything",
            vec![],
        ),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert_eq!(
            result.statements[0].status,
            AnalysisStatus::Complete,
            "{sql}"
        );
    }
    let tree = tree_sitter::parse_2way(
        "WITH c AS (SELECT id FROM orders) SELECT * FROM c JOIN users USING (id)",
    )
    .unwrap();
    assert_eq!(
        resolution::requests(&tree.root_node())
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        ["orders", "users"]
    );
}

#[test]
fn unmodelled_from_items_leave_the_statement_partially_checked() {
    for sql in [
        "SELECT nmae FROM users TABLESAMPLE SYSTEM (1); SELECT agge FROM users",
        "SELECT nmae FROM ONLY users; SELECT agge FROM users",
        "SELECT nmae FROM users AS u(a, b); SELECT agge FROM users",
        "SELECT nmae FROM (users JOIN orders USING (id)) AS j; SELECT agge FROM users",
        "SELECT nmae FROM users u, LATERAL (SELECT u.idd) s; SELECT agge FROM users",
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            ["agge"],
            "{sql}: {result:?}"
        );
        assert_eq!(
            result.statements[0].exclusion,
            Some(Exclusion::UnsupportedSyntax),
            "{sql}"
        );
        assert_eq!(result.statements[1].status, AnalysisStatus::Complete);
    }
    let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
    let result = Linter::new()
        .run_with_catalog(
            "SELECT 1 FROM users TABLESAMPLE SYSTEM (1), orders",
            &config(),
            Err(&error),
        )
        .unwrap();
    assert_eq!(result.statements[0].status, AnalysisStatus::Failed(error));
    let result = run("SELECT id FROM pg_temp.users");
    assert_eq!(
        result.statements[0].exclusion,
        Some(Exclusion::TemporarySchema)
    );
}

#[test]
fn output_names_bind_only_whole_items_and_derived_names_are_not_guessed() {
    for (sql, expected) in [
        // PostgreSQL names an expression output after its function or column.
        ("SELECT COALESCE(1, 2) ORDER BY coalesce", vec![]),
        ("SELECT id::text FROM users ORDER BY id", vec![]),
        ("SELECT COALESCE(id, 0) FROM users ORDER BY nmae", vec![]),
        ("SELECT id AS a FROM users ORDER BY a, nmae", vec!["nmae"]),
        ("SELECT id FROM users ORDER BY nmae", vec!["nmae"]),
        // Inside an expression only input columns are visible.
        ("SELECT id AS a FROM users ORDER BY a + 1", vec!["a"]),
        ("SELECT id AS a FROM users GROUP BY a + 1", vec!["a"]),
        ("SELECT DISTINCT ON (a + 1) id AS a FROM users", vec!["a"]),
        ("SELECT id AS a FROM users GROUP BY a", vec![]),
        // Parentheses do not make an expression, and grouping lists hold whole items.
        ("SELECT id AS a FROM users ORDER BY (a)", vec![]),
        ("SELECT id AS a FROM users GROUP BY (a)", vec![]),
        ("SELECT DISTINCT ON ((a)) id AS a FROM users", vec![]),
        ("SELECT id AS a FROM users GROUP BY ROLLUP (a)", vec![]),
        ("SELECT id AS a FROM users GROUP BY CUBE (a, id)", vec![]),
        (
            "SELECT id AS a FROM users GROUP BY GROUPING SETS ((a), (a, id), ())",
            vec![],
        ),
        ("SELECT id AS a FROM users GROUP BY (a, id)", vec![]),
        (
            "SELECT id AS a, id AS b, id AS c FROM users GROUP BY (a, b, c)",
            vec![],
        ),
        ("SELECT id AS a FROM users GROUP BY ((a, id))", vec![]),
        (
            "SELECT id AS a FROM users GROUP BY ROLLUP ((a, id))",
            vec![],
        ),
        (
            "SELECT id AS a FROM users GROUP BY CUBE ((a, id), (id))",
            vec![],
        ),
        (
            "SELECT id AS a FROM users GROUP BY ROLLUP (a + 1)",
            vec!["a"],
        ),
        (
            "SELECT id AS a FROM users ORDER BY (a, id, name)",
            vec!["a"],
        ),
        ("SELECT DISTINCT ON ((a, id)) id AS a FROM users", vec!["a"]),
        // Inside an expression list or row the alias is not visible.
        (
            "SELECT id AS a FROM users ORDER BY coalesce(a, 1)",
            vec!["a"],
        ),
        (
            "SELECT id AS a FROM users GROUP BY coalesce(a, 1)",
            vec!["a"],
        ),
        ("SELECT id AS a FROM users ORDER BY id IN (a)", vec!["a"]),
        ("SELECT id AS a FROM users ORDER BY ARRAY[a]", vec!["a"]),
        ("SELECT id AS a FROM users ORDER BY (a, id)", vec!["a"]),
        (
            "SELECT id AS a FROM users ORDER BY a COLLATE \"C\"",
            vec!["a"],
        ),
        ("SELECT id AS a FROM users ORDER BY a::int", vec!["a"]),
        ("SELECT id AS a FROM users HAVING a > 1", vec!["a"]),
        ("SELECT id AS a FROM users ORDER BY (SELECT a)", vec!["a"]),
        ("SELECT id AS \"A\" FROM users ORDER BY a", vec!["a"]),
        // `*` keeps the output names complete.
        ("SELECT * FROM users ORDER BY nmae", vec!["nmae"]),
        ("SELECT u.* FROM users u ORDER BY nmae", vec!["nmae"]),
        ("SELECT DISTINCT ON (a) id AS a FROM users", vec![]),
    ] {
        let result = run(sql);
        assert_eq!(slices(sql, &result.diagnostics), expected, "{sql}");
    }
}

#[test]
fn cte_names_follow_definition_order_and_nesting() {
    for (sql, expected) in [
        // A non-recursive CTE body sees the real table of the same name.
        (
            "WITH users AS (SELECT missing FROM users) SELECT * FROM users",
            vec!["missing"],
        ),
        (
            "WITH RECURSIVE users AS (SELECT missing FROM users) SELECT * FROM users",
            vec![],
        ),
        // A CTE is visible only to the bodies after it.
        (
            "WITH b AS (SELECT x FROM usres), usres AS (SELECT 1 x) SELECT 1",
            vec!["usres"],
        ),
        (
            "WITH usres AS (SELECT 1 x), b AS (SELECT x FROM usres) SELECT x FROM b",
            vec![],
        ),
        (
            "WITH RECURSIVE b AS (SELECT x FROM usres), usres AS (SELECT 1 x) SELECT 1",
            vec![],
        ),
        (
            "WITH c AS (SELECT (SELECT 1 FROM usres) x), usres AS (SELECT 1) SELECT 1",
            vec!["usres"],
        ),
        (
            "SELECT * FROM (WITH usres AS (SELECT 1) SELECT 1) d, usres",
            vec!["usres"],
        ),
        // A CTE nested in another CTE body is not visible to the outer query.
        (
            "WITH o AS (WITH usres AS (SELECT 1 x) SELECT x FROM usres) SELECT id FROM usres",
            vec!["usres"],
        ),
    ] {
        let result = run(sql);
        assert_eq!(slices(sql, &result.diagnostics), expected, "{sql}");
    }
}

fn bindings(sql: &str) -> Vec<(String, Resolution<Referent>)> {
    let tree = tree_sitter::parse_2way(sql).unwrap();
    let root = tree.root_node();
    let snapshot = snapshot();
    let catalog = Catalog::new(&root, Ok(&snapshot));
    root.descendants()
        .filter(|node| node.kind() == postgresql_cst_parser::syntax_kind::SyntaxKind::columnref)
        .map(|node| {
            let range = node.range();
            (
                sql[range.start_byte..range.end_byte].to_string(),
                catalog.binding(&node),
            )
        })
        .collect()
}

fn column(table: &str, column: &str) -> Resolution<Referent> {
    Resolution::Resolved(Referent::Column {
        schema: "public".into(),
        table: table.into(),
        column: column.into(),
    })
}

#[test]
fn bindings_name_what_each_reference_refers_to() {
    let row = |table: &str| Referent::WholeRow {
        schema: "public".into(),
        table: table.into(),
    };
    let all = |table: &str| Referent::AllColumns {
        schema: "public".into(),
        table: table.into(),
    };
    let output = |name: &str| Resolution::Resolved(Referent::Output { name: name.into() });
    let unknown = |reason| Resolution::Unknown(ResolutionUnknown::Reason(reason));
    for (sql, expected) in [
        (
            "SELECT u.id, name, u, u.* FROM users u",
            vec![
                ("u.id", column("users", "id")),
                ("name", column("users", "name")),
                ("u", Resolution::Resolved(row("users"))),
                ("u.*", Resolution::Resolved(all("users"))),
            ],
        ),
        (
            "SELECT id, amount FROM users, orders",
            vec![
                ("id", Resolution::Ambiguous),
                ("amount", column("orders", "amount")),
            ],
        ),
        (
            "SELECT nmae, x.id FROM users u",
            vec![
                ("nmae", Resolution::Absent(AbsenceKind::Column)),
                ("x.id", Resolution::Absent(AbsenceKind::Table)),
            ],
        ),
        // ORDER BY prefers an output name, GROUP BY an input column.
        (
            "SELECT name AS id FROM users ORDER BY id",
            vec![("name", column("users", "name")), ("id", output("id"))],
        ),
        (
            "SELECT name AS id FROM users GROUP BY id",
            vec![
                ("name", column("users", "name")),
                ("id", column("users", "id")),
            ],
        ),
        (
            "SELECT id AS a FROM users ORDER BY a",
            vec![("id", column("users", "id")), ("a", output("a"))],
        ),
        // Correlation binds to the enclosing FROM item.
        (
            "SELECT 1 FROM users u WHERE EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id)",
            vec![
                ("o.user_id", column("orders", "user_id")),
                ("u.id", column("users", "id")),
            ],
        ),
        (
            "WITH c AS (SELECT id FROM users) SELECT x FROM c",
            vec![
                ("id", column("users", "id")),
                ("x", unknown(UnknownReason::UnsupportedRelation)),
            ],
        ),
        (
            "SELECT COALESCE(1, 2) ORDER BY coalesce",
            vec![("coalesce", unknown(UnknownReason::IncompleteCoverage))],
        ),
        (
            "SELECT lower(name) FROM users",
            vec![("name", unknown(UnknownReason::UnsupportedSyntax))],
        ),
    ] {
        let expected = expected
            .into_iter()
            .map(|(text, resolution)| (text.to_string(), resolution))
            .collect::<Vec<_>>();
        assert_eq!(bindings(sql), expected, "{sql}");
    }
}

#[test]
fn decision_table_edges_of_reference_binding() {
    for (sql, expected) in [
        // Expression shapes that are not function applications are still checked.
        (
            "SELECT coalesce(nmae, 'x'), EXTRACT(YEAR FROM agge), ARRAY[missing], ROW(id, other) FROM users",
            vec!["nmae", "agge", "missing", "other"],
        ),
        // Without FROM, nothing is visible.
        ("SELECT missing", vec!["missing"]),
        ("SELECT 1 FROM users WHERE EXISTS (SELECT nmae)", vec!["nmae"]),
        // Correlation from the target list, HAVING, ORDER BY and set-operation arms.
        (
            "SELECT (SELECT o.amount FROM orders o WHERE o.user_id = u.idd) FROM users u",
            vec!["idd"],
        ),
        (
            "SELECT id FROM users u GROUP BY id HAVING EXISTS (SELECT 1 FROM orders o WHERE o.user_id = u.id)",
            vec![],
        ),
        (
            "SELECT id FROM users u ORDER BY (SELECT o.amount FROM orders o WHERE o.user_id = u.id)",
            vec![],
        ),
        (
            "SELECT 1 FROM users u WHERE u.id IN (SELECT id FROM orders UNION SELECT u.agge)",
            vec!["agge"],
        ),
        // Output names are visible only to the ORDER BY of their own query.
        ("SELECT id AS x FROM users ORDER BY (SELECT x)", vec!["x"]),
        ("SELECT 'x' AS nmae FROM users ORDER BY nmae", vec![]),
        // Whole-row references use the visible name only.
        ("SELECT users FROM users u", vec!["users"]),
        ("SELECT users FROM users", vec![]),
        // Subscripts do not change the referenced column.
        ("SELECT nmae[1], u.nmae[1:2], id[1] FROM users u", vec!["nmae", "nmae"]),
        ("SELECT id FROM users GROUP BY ROLLUP (missing)", vec!["missing"]),
        (
            "SELECT id FROM users GROUP BY GROUPING SETS ((missing), ())",
            vec!["missing"],
        ),
        ("SELECT 1 FROM users WHERE EXISTS (SELECT 1 FROM usres)", vec!["usres"]),
        ("SELECT * FROM users CROSS JOIN usres", vec!["usres"]),
        // A column in both tables is ambiguous, which is not this rule's concern.
        ("SELECT id FROM users, orders", vec![]),
        // Not modelled: VALUES, WINDOW definitions, ORDER BY of a parenthesized query.
        ("VALUES (missing)", vec![]),
        ("SELECT id FROM users WHERE id IN (VALUES (missing))", vec![]),
        ("SELECT id FROM users WINDOW w AS (PARTITION BY missing)", vec![]),
        ("(SELECT id FROM users) ORDER BY nmae", vec![]),
        // CTE-like syntax that the resolver must not confuse with a table.
        (
            "WITH users AS (SELECT 1 AS one) SELECT one FROM public.users",
            vec!["one"],
        ),
        (
            "WITH RECURSIVE t AS (SELECT 1 AS n UNION ALL SELECT n + 1 FROM t WHERE n < 10) SELECT n, nmae FROM t",
            vec![],
        ),
        ("WITH c(a) AS (SELECT id FROM users) SELECT a, nmae FROM c", vec![]),
    ] {
        let result = run(sql);
        assert_eq!(slices(sql, &result.diagnostics), expected, "{sql}: {result:?}");
        assert!(
            result
                .statements
                .iter()
                .all(|s| s.status == AnalysisStatus::Complete),
            "{sql}"
        );
    }
}

#[test]
fn decision_table_edges_of_statement_coverage() {
    // A join alias over USING hides the joined names; it is not modelled.
    for sql in [
        "SELECT j.id FROM users JOIN orders USING (id) AS j",
        "SELECT id FROM users u JOIN orders o USING (id) AS j WHERE j.id = 1",
    ] {
        let result = run(sql);
        assert!(result.diagnostics.is_empty(), "{sql}: {result:?}");
        assert_eq!(
            result.statements[0].exclusion,
            Some(Exclusion::UnsupportedSyntax),
            "{sql}"
        );
    }
    // A qualified reference to a found table is still checked next to unmodelled items.
    let sql = "SELECT u.nmae, missing FROM users u, LATERAL (SELECT u.id) s";
    assert_eq!(slices(sql, &run(sql).diagnostics), ["nmae"]);
    // An unmodelled FROM item is neither requested nor reported as absent.
    let sql = "SELECT 1 FROM usres TABLESAMPLE SYSTEM (1); SELECT 1 FROM ONLY usres";
    let result = run(sql);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    let tree = tree_sitter::parse_2way(sql).unwrap();
    assert!(resolution::requests(&tree.root_node()).is_empty());
    // Any function application holds the whole input.
    let sql = "SELECT lower(nmae) FROM users; SELECT agge FROM users";
    let result = run(sql);
    assert!(result.diagnostics.is_empty(), "{result:?}");
    assert!(result
        .statements
        .iter()
        .all(|s| s.exclusion == Some(Exclusion::FileEffect)));
    // A recovered source does not hide an absent sibling table.
    let sql = "SELECT x.id FROM /*#table*/ AS x, usres";
    let result = run(sql);
    assert_eq!(slices(sql, &result.diagnostics), ["usres"]);
    assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    assert!(result.statements[0].recovered_source);
    // Unsupported table identifiers leave the statement not fully checked.
    let long = "a".repeat(64);
    for sql in [
        format!("SELECT id FROM {long}"),
        format!("SELECT id FROM users AS {long}"),
    ] {
        let result = run(&sql);
        assert!(result.diagnostics.is_empty(), "{sql}");
        assert_eq!(
            result.statements[0].exclusion,
            Some(Exclusion::UnsupportedIdentifier),
            "{sql}"
        );
    }
    // Requests are distinct per written schema and include every nested query.
    let tree = tree_sitter::parse_2way(
        "SELECT 1 FROM users, public.users, \"users\", app.users, (SELECT id FROM orders) s",
    )
    .unwrap();
    assert_eq!(
        resolution::requests(&tree.root_node())
            .iter()
            .map(|r| (r.schema.as_deref(), r.name.as_str()))
            .collect::<Vec<_>>(),
        [
            (None, "users"),
            (Some("public"), "users"),
            (Some("app"), "users"),
            (None, "orders")
        ]
    );
}

#[test]
fn wildcard_policy_warning_remains_independent_of_catalog_resolution() {
    let sql = "SELECT x.*, missing FROM users u";
    let mut config = config();
    config.rules.push((
        RuleEnum::NoWildcardProjection(crate::rules::NoWildcardProjection),
        Severity::Warning,
    ));
    let result = Linter::new()
        .run_with_catalog(sql, &config, Ok(&snapshot()))
        .unwrap();
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "no-wildcard-projection"));
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "no-unknown-reference")
            .map(|d| &sql[d.span.start.byte..d.span.end.byte])
            .collect::<Vec<_>>(),
        ["x", "missing"]
    );
}

#[tokio::test]
async fn accepted_sql_runs_from_prepared_requests_through_memory_provider() {
    for sql in [
        "SELECT id, name FROM users;",
        "SELECT u.id AS user_id FROM public.users AS u WHERE u.age >= 18 AND u.name IS NOT NULL;",
        "SELECT (age + 1) AS next_age FROM users WHERE NOT (age < 18 OR name IS NULL);",
        "SELECT u, u AS row_value FROM users AS u WHERE u IS NOT NULL;",
        "SELECT ctid FROM users;",
        "SELECT , id FROM , users WHERE AND id = /*param*/ ;",
        "/*IF false*/ SELECT /*param*/ AS result, id FROM users; /*END*/",
    ] {
        let tree = tree_sitter::parse_2way(sql).unwrap();
        let requests = resolution::requests(&tree.root_node());
        let acquired = InMemoryCatalogProvider::new(snapshot())
            .acquire(&requests)
            .await
            .unwrap();
        let result = Linter::new()
            .run_with_catalog(sql, &config(), Ok(&acquired))
            .unwrap();
        assert!(result.diagnostics.is_empty(), "{sql}: {result:?}");
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
}

#[test]
fn acceptance_examples_diagnose_only_the_original_reference() {
    for (sql, expected) in [
        ("SELECT nmae FROM users;", vec!["nmae"]),
        ("SELECT id FROM users WHERE agge > 18;", vec!["agge"]),
        ("SELECT id FROM usres;", vec!["usres"]),
        ("SELECT id FROM /*$table*/ usres;", vec!["usres"]),
        ("SELECT x.id FROM users AS u;", vec!["x"]),
        (
            "SELECT id AS user_id FROM users WHERE user_id = 1;",
            vec!["user_id"],
        ),
        (
            "SELECT x.nmae, users.id, u.nmae FROM users u;",
            vec!["x", "users", "nmae"],
        ),
        (
            "SELECT x.nmae, users.id FROM usres u WHERE missing > 1;",
            vec!["usres"],
        ),
        ("SELECT count(*) FROM users;", vec![]),
        ("SELECT s.id FROM (SELECT id FROM users) AS s;", vec![]),
    ] {
        let r = run(sql);
        assert_eq!(slices(sql, &r.diagnostics), expected, "{sql}");
        assert!(r
            .diagnostics
            .iter()
            .all(|d| d.code == "no-unknown-reference" && d.severity == Severity::Error));
    }
}

#[test]
fn bounded_clauses_keep_reference_ranges_and_implicit_aliases() {
    for (sql, expected) in [
        ("SELECT nmae FROM users LIMIT 1 OFFSET 2", vec!["nmae"]),
        (
            "SELECT nmae FROM users WHERE agge > 1 LIMIT 1 OFFSET 2",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT nmae FROM users OFFSET 2 ROWS FETCH NEXT 1 ROW ONLY",
            vec!["nmae"],
        ),
        (
            "SELECT nmae FROM users WHERE agge > 0 LIMIT ALL",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT nmae FROM users WHERE agge > 0 FETCH FIRST ROW ONLY",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT nmae FROM users WHERE agge > 0 FETCH FIRST ROWS ONLY",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT nmae FROM users WHERE agge > 0 FETCH NEXT ROW ONLY",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT nmae FROM users WHERE agge > 0 FETCH NEXT ROWS ONLY",
            vec!["nmae", "agge"],
        ),
        ("SELECT DISTINCT nmae FROM users", vec!["nmae"]),
        ("SELECT id alias FROM users WHERE alias = 1", vec!["alias"]),
        ("SELECT u.nmae FROM users u FOR UPDATE OF u", vec!["nmae"]),
    ] {
        let result = run(sql);
        assert_eq!(slices(sql, &result.diagnostics), expected, "{sql}");
        if sql.contains(" LIMIT ALL")
            || sql.contains(" FETCH FIRST ROW")
            || sql.contains(" FETCH NEXT ROW")
        {
            assert_eq!(
                result
                    .diagnostics
                    .iter()
                    .map(|d| d.span.start.byte)
                    .collect::<Vec<_>>(),
                [sql.find("nmae").unwrap(), sql.find("agge").unwrap()],
                "{sql}"
            );
        }
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
    let result = run("SELECT missing FROM users FOR UPDATE OF users");
    assert_eq!(result.diagnostics[0].code, "no-unknown-reference");
    assert_eq!(
        slices(
            "SELECT missing FROM users FOR UPDATE OF users",
            &result.diagnostics
        ),
        ["missing"]
    );
}

#[test]
fn valueless_limit_forms_keep_adjacent_statement_boundaries() {
    for clause in [
        "LIMIT ALL",
        "FETCH FIRST ROW ONLY",
        "FETCH FIRST ROWS ONLY",
        "FETCH NEXT ROW ONLY",
        "FETCH NEXT ROWS ONLY",
    ] {
        let sql = format!(
            "SELECT nmae FROM users {clause}; SELECT id FROM users LIMIT missing; SELECT agge FROM users"
        );
        let result = run(&sql);
        assert_eq!(slices(&sql, &result.diagnostics), ["nmae", "agge"], "{sql}");
        assert_eq!(
            result.diagnostics[0].span.start.byte,
            sql.find("nmae").unwrap()
        );
        assert_eq!(
            result.diagnostics[1].span.start.byte,
            sql.find("agge").unwrap()
        );
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
        assert_eq!(result.statements[1].status, AnalysisStatus::Complete);
        assert_eq!(result.statements[2].status, AnalysisStatus::Complete);
    }
}

#[test]
fn large_and_separated_integer_clauses_keep_catalog_diagnostics() {
    for sql in [
        "SELECT nmae FROM users LIMIT 2147483648",
        "SELECT nmae FROM users LIMIT 1_000",
        "SELECT nmae FROM users OFFSET 2147483648",
        "SELECT nmae FROM users OFFSET 1_000 ROWS",
        "SELECT nmae FROM users FETCH FIRST 2147483648 ROW ONLY",
        "SELECT nmae FROM users FETCH NEXT 1_000 ROWS ONLY",
    ] {
        let result = run(sql);
        assert_eq!(slices(sql, &result.diagnostics), ["nmae"], "{sql}");
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
    for value in ["1.0", "1e3", "1 + 1"] {
        let sql = format!("SELECT nmae FROM users LIMIT {value}; SELECT agge FROM users");
        let result = run(&sql);
        assert_eq!(slices(&sql, &result.diagnostics), ["nmae", "agge"], "{sql}");
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
}

#[test]
fn every_select_clause_is_checked_except_limit_values_and_locking_targets() {
    for (statement, expected) in [
        (
            "SELECT DISTINCT ON (missing) id FROM users",
            vec!["missing"],
        ),
        ("SELECT DISTINCT ON (id) id FROM users", vec![]),
        // LIMIT cannot refer to columns at all, so an unknown name there is not a catalog error.
        ("SELECT id FROM users LIMIT missing", vec![]),
        ("SELECT id FROM users FETCH FIRST ROW WITH TIES", vec![]),
        ("SELECT id FROM users FOR UPDATE OF other", vec![]),
        ("SELECT id FROM users u FOR UPDATE OF users", vec![]),
        ("SELECT id FROM users FOR UPDATE SKIP LOCKED", vec![]),
    ] {
        let sql = format!("{statement}; SELECT nmae FROM users");
        let result = run(&sql);
        let expected = expected.into_iter().chain(["nmae"]).collect::<Vec<_>>();
        assert_eq!(slices(&sql, &result.diagnostics), expected, "{sql}");
        assert!(
            result
                .statements
                .iter()
                .all(|s| s.status == AnalysisStatus::Complete),
            "{sql}"
        );
    }
}

#[test]
fn bounded_expressions_diagnose_all_value_references_at_original_ranges() {
    for (sql, expected) in [
        (
            "SELECT missing IN (age, other) FROM users",
            vec!["missing", "other"],
        ),
        (
            "SELECT id NOT IN (other, more) FROM users",
            vec!["other", "more"],
        ),
        (
            "SELECT id IN (other + more, age) FROM users",
            vec!["other", "more"],
        ),
        (
            "SELECT id BETWEEN low + age AND high FROM users",
            vec!["low", "high"],
        ),
        (
            "SELECT id NOT BETWEEN -low AND high FROM users",
            vec!["low", "high"],
        ),
        ("SELECT name LIKE pattern FROM users", vec!["pattern"]),
        ("SELECT name ILIKE pattern FROM users", vec!["pattern"]),
        ("SELECT name NOT LIKE pattern FROM users", vec!["pattern"]),
        ("SELECT name NOT ILIKE pattern FROM users", vec!["pattern"]),
        (
            "SELECT missing LIKE other FROM users",
            vec!["missing", "other"],
        ),
        ("SELECT name || suffix FROM users", vec!["suffix"]),
        (
            "SELECT missing || other FROM users",
            vec!["missing", "other"],
        ),
        (
            "SELECT missing::text, CAST(other AS integer) FROM users",
            vec!["missing", "other"],
        ),
        ("SELECT (missing + age)::text FROM users", vec!["missing"]),
        (
            "SELECT id FROM users WHERE missing IN (age, other)",
            vec!["missing", "other"],
        ),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
    let result = run("SELECT id::missing_type, CAST(age AS another_type) FROM users");
    assert!(result.diagnostics.is_empty(), "{result:?}");
}

#[test]
fn between_lower_cast_and_concat_keep_all_reference_ranges_and_unknowns() {
    for (sql, expected) in [
        (
            "SELECT id BETWEEN low::text || other AND high FROM users",
            vec!["low", "other", "high"],
        ),
        (
            "SELECT id NOT BETWEEN CAST(low AS text) || other AND high FROM users",
            vec!["low", "other", "high"],
        ),
        (
            "SELECT id BETWEEN (low::text || other) AND high FROM users",
            vec!["low", "other", "high"],
        ),
        (
            "SELECT id FROM users WHERE id BETWEEN low::text || other AND high",
            vec!["low", "other", "high"],
        ),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }

    let sql = "SELECT id BETWEEN low::text || other AND high FROM usres";
    assert_eq!(slices(sql, &run(sql).diagnostics), ["usres"]);

    let sql = "SELECT x.id BETWEEN x.low::text || x.other AND x.high FROM /*#table*/ AS x";
    let tree = tree_sitter::parse_2way(sql).unwrap();
    assert!(resolution::requests(&tree.root_node()).is_empty());
    assert!(run(sql).diagnostics.is_empty());

    let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
    let sql = "SELECT id BETWEEN low::text || other AND high FROM users";
    let result = Linter::new()
        .run_with_catalog(sql, &config(), Err(&error))
        .unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.statements[0].status, AnalysisStatus::Failed(error));

    for (statement, expected) in [
        (
            "SELECT id BETWEEN low::varchar(10) AND high FROM users",
            vec!["low", "high"],
        ),
        (
            "SELECT id BETWEEN low ## other AND high FROM users",
            vec!["low", "other", "high"],
        ),
    ] {
        let sql = format!("{statement}; SELECT nmae FROM users");
        let result = run(&sql);
        let expected = expected.into_iter().chain(["nmae"]).collect::<Vec<_>>();
        assert_eq!(slices(&sql, &result.diagnostics), expected, "{sql}");
    }
}

#[test]
fn bounded_expression_unknown_sources_do_not_create_derived_absence() {
    let sql = "SELECT missing IN (other, age) FROM usres";
    let result = run(sql);
    assert_eq!(slices(sql, &result.diagnostics), ["usres"]);

    let sql = "SELECT x.missing BETWEEN other AND age FROM /*#table*/ AS x";
    let tree = tree_sitter::parse_2way(sql).unwrap();
    assert!(resolution::requests(&tree.root_node()).is_empty());
    assert!(run(sql).diagnostics.is_empty());

    let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
    let sql = "SELECT missing || other FROM users";
    let result = Linter::new()
        .run_with_catalog(sql, &config(), Err(&error))
        .unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.statements[0].status, AnalysisStatus::Failed(error));
}

#[test]
fn any_value_expression_shape_diagnoses_its_column_references() {
    for (statement, expected) in [
        (
            "SELECT missing IN (SELECT id FROM users) FROM users",
            vec!["missing"],
        ),
        (
            "SELECT name LIKE pattern ESCAPE esc FROM users",
            vec!["pattern", "esc"],
        ),
        ("SELECT name SIMILAR TO pattern FROM users", vec!["pattern"]),
        ("SELECT name ## suffix FROM users", vec!["suffix"]),
        ("SELECT missing::varchar(10) FROM users", vec!["missing"]),
        (
            "SELECT CAST(missing AS numeric(10,2)) FROM users",
            vec!["missing"],
        ),
        ("SELECT missing::int[] FROM users", vec!["missing"]),
        ("SELECT missing::public.custom FROM users", vec!["missing"]),
        (
            "SELECT CASE WHEN missing THEN name END FROM users",
            vec!["missing"],
        ),
        // Three-part names are not modelled.
        ("SELECT public.users.* FROM users", vec![]),
        ("SELECT public.users.id FROM users", vec![]),
    ] {
        let sql = format!("{statement}; SELECT nmae FROM users");
        let result = run(&sql);
        let expected = expected.into_iter().chain(["nmae"]).collect::<Vec<_>>();
        assert_eq!(
            slices(&sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
}

#[test]
fn qualified_table_and_utf8_crlf_repeated_identifiers_have_original_spans() {
    let sql = "-- 普通のコメント\r\nSELECT \"a\"\"b\", \"名前\", nmae, nmae FROM users;\r\nSELECT id FROM public.usres;";
    let r = run(sql);
    assert_eq!(
        slices(sql, &r.diagnostics),
        ["\"a\"\"b\"", "\"名前\"", "nmae", "nmae", "public.usres"]
    );
    for diagnostic in &r.diagnostics {
        let prefix = &sql[..diagnostic.span.start.byte];
        assert_eq!(
            diagnostic.span.start.line,
            prefix.bytes().filter(|b| *b == b'\n').count()
        );
        assert_eq!(
            diagnostic.span.start.column,
            prefix.rsplit('\n').next().unwrap().len()
        );
    }
    assert!(r
        .diagnostics
        .windows(2)
        .all(|d| d[0].span.start.byte < d[1].span.start.byte));
}

#[test]
fn quoted_case_and_real_column_whole_row_precedence() {
    let s = CatalogSnapshot::new(
        vec!["public".into()],
        [CatalogEntry {
            schema: "public".into(),
            table: "Users".into(),
            outcome: Lookup::Found(table("public", "Users", &["Id", "U"])),
        }],
    )
    .unwrap();
    let sql = "SELECT \"Id\", id, \"U\", u FROM public.\"Users\" AS \"U\"";
    let result = Linter::new()
        .run_with_catalog(sql, &config(), Ok(&s))
        .unwrap();
    assert_eq!(slices(sql, &result.diagnostics), ["id", "u"]);
}

#[test]
fn failures_and_unknown_sources_never_become_absence() {
    let error = AcquisitionError::new(
        AcquisitionPhase::Schema,
        AcquisitionErrorKind::PermissionDenied,
    );
    for outcome in [
        Lookup::Unknown(UnknownReason::UnsupportedRelation),
        Lookup::Unknown(UnknownReason::IncompleteCoverage),
        Lookup::Unavailable(error.clone()),
    ] {
        let s = CatalogSnapshot::new(
            vec!["public".into()],
            [CatalogEntry {
                schema: "public".into(),
                table: "users".into(),
                outcome,
            }],
        )
        .unwrap();
        let r = Linter::new()
            .run_with_catalog("SELECT x.missing, unknown FROM users u", &config(), Ok(&s))
            .unwrap();
        assert!(r.diagnostics.is_empty());
        assert_ne!(r.statements[0].status, AnalysisStatus::Complete);
    }
    let mut cfg = config();
    cfg.rules
        .push((RuleEnum::NoDistinct(NoDistinct), Severity::Warning));
    let r = Linter::new()
        .run_with_catalog(
            "SELECT DISTINCT ON (id) id FROM users; SELECT missing FROM users",
            &cfg,
            Err(&error),
        )
        .unwrap();
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].code, "no-distinct");
    assert_eq!(
        r.statements[0].status,
        AnalysisStatus::Failed(error.clone())
    );
    assert_eq!(r.statements[1].status, AnalysisStatus::Failed(error));
}

#[test]
fn file_effect_suppresses_catalog_diagnostics_but_keeps_cst_diagnostics() {
    let sql =
        "SELECT DISTINCT nmae FROM users; CREATE TABLE scratch(id integer); SELECT agge FROM users";
    let mut cfg = config();
    cfg.rules
        .push((RuleEnum::NoDistinct(NoDistinct), Severity::Warning));
    let result = Linter::new()
        .run_with_catalog(sql, &cfg, Ok(&snapshot()))
        .unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "no-distinct");
    assert!(result.statements.iter().all(|statement| {
        matches!(statement.status, AnalysisStatus::Excluded(_))
            && statement.exclusion == Some(Exclusion::FileEffect)
    }));

    for effect in [
        "SET CONSTRAINTS ALL DEFERRED",
        "SELECT set_config('search_path', 'public', false) FROM users",
        "SELECT id INTO scratch FROM users",
        "WITH changed AS (UPDATE users SET id = 1 RETURNING id) SELECT id FROM users",
    ] {
        let sql = format!("SELECT nmae FROM users; {effect}; SELECT agge FROM users");
        let tree = tree_sitter::parse_2way(&sql).unwrap();
        assert!(resolution::requests(&tree.root_node()).is_empty(), "{sql}");
        let result = run(&sql);
        assert!(result.diagnostics.is_empty(), "{sql}: {result:?}");
        assert!(result
            .statements
            .iter()
            .all(|s| s.exclusion == Some(Exclusion::FileEffect)));
    }
}

#[test]
fn suppression_preserves_other_rules_lines_and_internal_resolution() {
    let mut cfg = config();
    cfg.rules
        .push((RuleEnum::NoDistinct(NoDistinct), Severity::Warning));
    let sql = "-- uroborosql-lint-disable-next-line no-unknown-reference\nSELECT , nmae FROM users;\nSELECT nmae FROM users; SELECT DISTINCT ON (id) id FROM users;";
    let r = Linter::new()
        .run_with_catalog(sql, &cfg, Ok(&snapshot()))
        .unwrap();
    assert_eq!(
        r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["no-unknown-reference", "no-distinct"]
    );
    assert_eq!(r.diagnostics[0].span.start.line, 2);
    assert_eq!(r.statements[0].status, AnalysisStatus::Complete);
    let sql = "-- uroborosql-lint-disable no-unknown-reference\nSELECT , nmae FROM users; SELECT DISTINCT ON (id) id FROM users;";
    let r = Linter::new()
        .run_with_catalog(sql, &cfg, Ok(&snapshot()))
        .unwrap();
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].code, "no-distinct");
    assert_eq!(r.statements[1].status, AnalysisStatus::Complete);
    let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
    let r = Linter::new()
        .run_with_catalog(sql, &cfg, Err(&error))
        .unwrap();
    assert_eq!(r.statements[0].status, AnalysisStatus::Failed(error));
}

#[test]
fn invalid_directive_is_reported_once_with_cst_and_catalog_diagnostics() {
    let mut cfg = config();
    cfg.rules
        .push((RuleEnum::NoDistinct(NoDistinct), Severity::Warning));
    let r = Linter::new().run_with_catalog("-- uroborosql-lint-disable invalid-rule\nSELECT , nmae FROM users; SELECT DISTINCT id FROM users;",&cfg,Ok(&snapshot())).unwrap();
    assert_eq!(
        r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        [
            "invalid-lint-directive",
            "no-unknown-reference",
            "no-distinct"
        ]
    );
    let r = Linter::new()
        .run_with_catalog(
            "-- uroborosql-lint-disable\nSELECT nmae FROM users; SELECT DISTINCT id FROM users;",
            &cfg,
            Ok(&snapshot()),
        )
        .unwrap();
    assert_eq!(
        r.diagnostics
            .iter()
            .filter(|d| d.code == "invalid-lint-directive")
            .count(),
        1
    );
}

#[test]
fn configuration_severity_off_and_file_overrides_apply_to_registered_rule() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(&path, r#"{"rules":{"no-unknown-reference":"warn"},"overrides":[{"files":["off.sql"],"rules":{"no-unknown-reference":"off"}},{"files":["error.sql"],"rules":{"no-unknown-reference":"error"}}]}"#).unwrap();
    let store = ConfigStore::try_new(dir.path(), Some(path))
        .unwrap()
        .unwrap();
    for (file, severity) in [
        ("warning.sql", Some(Severity::Warning)),
        ("off.sql", None),
        ("error.sql", Some(Severity::Error)),
    ] {
        let cfg = store.resolve(&dir.path().join(file));
        let r = Linter::new()
            .run_with_catalog("SELECT , nmae FROM users", &cfg, Ok(&snapshot()))
            .unwrap();
        assert_eq!(
            r.diagnostics
                .iter()
                .filter(|d| d.code == "no-unknown-reference")
                .map(|d| d.severity)
                .collect::<Vec<_>>(),
            severity.into_iter().collect::<Vec<_>>()
        );
    }
}

#[test]
fn existing_sync_entry_stays_cst_only_and_parse_errors_remain_errors() {
    assert!(Linter::new()
        .run("SELECT nmae FROM users", &ResolvedLintConfig::default())
        .unwrap()
        .is_empty());
    for sql in ["SELECT FROM ;", "SELECT id FROM users WHERE"] {
        assert!(Linter::new()
            .run_with_catalog(sql, &config(), Ok(&snapshot()))
            .is_err());
    }
}

#[test]
fn two_way_samples_and_ordinary_comments_keep_catalog_diagnostics() {
    for sample in ["-1", "(1)"] {
        let sql = format!("SELECT nmae FROM users WHERE id = /*id*/{sample}");
        let result = run(&sql);
        assert_eq!(slices(&sql, &result.diagnostics), ["nmae"]);
        assert_eq!(result.statements[0].status, AnalysisStatus::Complete);
    }
    let sql = "SELECT nmae FROM users WHERE id = /* ordinary comment */1";
    assert_eq!(slices(sql, &run(sql).diagnostics), ["nmae"]);
}

#[test]
fn successful_recovery_and_two_way_comments_resolve_static_references() {
    for (sql, expected) in [
        ("/*IF false*/ SELECT nmae FROM users; /*END*/", vec!["nmae"]),
        (
            "/*IF false*/ SELECT nmae FROM users; SELECT agge FROM users;",
            vec!["nmae", "agge"],
        ),
        ("/*END*/ SELECT nmae FROM users;", vec!["nmae"]),
        ("SELECT id FROM users WHERE id = /*param*/ 1;", vec![]),
        ("SELECT /*param*/ AS result, nmae FROM users;", vec!["nmae"]),
        (
            "SELECT id FROM users WHERE nmae = /*param*/ ;",
            vec!["nmae"],
        ),
        ("SELECT , nmae FROM users;", vec!["nmae"]),
        ("SELECT nmae FROM , users;", vec!["nmae"]),
        ("SELECT id FROM users WHERE AND agge = 1;", vec!["agge"]),
        ("SELECT id FROM users WHERE OR agge = 1;", vec!["agge"]),
        (
            "SELECT , , nmae FROM , , users WHERE AND OR agge = 1;",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT , /*param*/ AS result, nmae FROM , users WHERE AND agge = /*param*/ ;",
            vec!["nmae", "agge"],
        ),
        (
            "SELECT '', /* ordinary comment */ nmae FROM users;",
            vec!["nmae"],
        ),
        ("SELECT id FROM usres;", vec!["usres"]),
        ("SELECT id FROM /*$table*/ usres;", vec!["usres"]),
        ("SELECT , x.id, nmae FROM users;", vec!["x", "nmae"]),
    ] {
        let result = run(sql);
        assert_eq!(
            slices(sql, &result.diagnostics),
            expected,
            "{sql}: {result:?}"
        );
        assert!(
            result
                .statements
                .iter()
                .all(|s| s.status == AnalysisStatus::Complete),
            "{sql}"
        );
    }
}

#[test]
fn recovered_sources_preserve_partial_results_without_absence_or_acquisition_failure() {
    for sql in [
        "SELECT x.id AS result, x AS result, wrong.nmae FROM /*#table*/ AS x WHERE x.id = 1;",
        "SELECT x.id AS result, x AS result, wrong.nmae FROM /*$table*/ AS x WHERE x.id = 1;",
        "SELECT , /*param*/ AS result, x AS result, wrong.nmae FROM , /*#table*/ AS x WHERE AND x.id = /*param*/ ;",
    ] {
        let tree = tree_sitter::parse_2way(sql).unwrap();
        assert!(resolution::requests(&tree.root_node()).is_empty());
        let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
        for acquired in [Ok(&snapshot()), Err(&error)] {
            let result = Linter::new().run_with_catalog(sql, &config(), acquired).unwrap();
            assert!(result.diagnostics.is_empty(), "{sql}: {result:?}");
            let statement = &result.statements[0];
            assert_eq!(statement.status, AnalysisStatus::Complete);
            assert!(statement.exclusion.is_none());
            assert!(statement.recovered_source);
        }
        let sql = format!("{sql} SELECT nmae FROM users;");
        let result = run(&sql);
        assert_eq!(slices(&sql, &result.diagnostics), ["nmae"]);
    }
}

#[test]
fn recovery_keeps_utf8_crlf_repeated_and_quoted_reference_ranges() {
    let sql = "/*IF false*/\r\nSELECT , , \"名前\", nmae, nmae FROM , users WHERE AND OR \"a\"\"b\" = /*param*/ ; /*END*/";
    let result = run(sql);
    assert_eq!(
        slices(sql, &result.diagnostics),
        ["\"名前\"", "nmae", "nmae", "\"a\"\"b\""]
    );
    for diagnostic in &result.diagnostics {
        assert_eq!(diagnostic.span.start.line, 1);
        assert_eq!(
            diagnostic.span.start.column,
            sql[..diagnostic.span.start.byte]
                .rsplit('\n')
                .next()
                .unwrap()
                .len()
        );
    }
    assert!(result
        .diagnostics
        .windows(2)
        .all(|pair| pair[0].span.start.byte < pair[1].span.start.byte));
}

/// Manual SQL inspection against the disposable PostgreSQL fixture.
#[cfg(feature = "postgres-catalog")]
#[tokio::test]
#[ignore = "use tests/postgres/run.py --review-sql PATH"]
async fn inspect_postgres_sql() {
    use crate::catalog::postgres::{PostgresCatalogProvider, PostgresConfig, TlsMode};
    use crate::diagnostic::OneBasedPosition;
    use std::{env, fs};

    let path = env::var("CATALOG_REVIEW_SQL").expect("set CATALOG_REVIEW_SQL to a SQL file");
    let sql = fs::read_to_string(&path).expect("read SQL file");
    let tree = tree_sitter::parse_2way(&sql).expect("parse SQL file");
    let requests = resolution::requests(&tree.root_node());
    let mut connection = PostgresConfig::new("127.0.0.1", "postgres", "postgres");
    connection.port = env::var("CATALOG_TEST_PORT")
        .expect("run tests/postgres/run.py")
        .parse()
        .expect("numeric fixture port");
    connection.password = Some(env::var("CATALOG_TEST_PASSWORD").expect("fixture password"));
    connection.tls_mode = TlsMode::Disable;
    let acquired = PostgresCatalogProvider::new(connection)
        .acquire(&requests)
        .await;
    if let Err(error) = &acquired {
        println!("catalog acquisition: {error}");
    }
    let result = Linter::new()
        .run_with_catalog(&sql, &config(), acquired.as_ref())
        .expect("lint SQL file");
    for (index, statement) in result.statements.iter().enumerate() {
        println!(
            "statement {}: {:?}; exclusion={:?}",
            index + 1,
            statement.status,
            statement.exclusion
        );
    }
    println!("diagnostics: {}", result.diagnostics.len());
    for diagnostic in result.diagnostics {
        println!(
            "{}:{}: {:?} {}: {} (source={:?})",
            path,
            OneBasedPosition::from_byte_offset(&sql, diagnostic.span.start.byte),
            diagnostic.severity,
            diagnostic.code,
            diagnostic.message,
            &sql[diagnostic.span.start.byte..diagnostic.span.end.byte]
        );
    }
}
