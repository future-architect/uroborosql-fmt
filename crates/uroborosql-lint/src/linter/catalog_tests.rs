use super::*;
use crate::{
    catalog::{
        AbsenceKind, AcquisitionErrorKind, AcquisitionPhase, CatalogEntry, CatalogProvider,
        ColumnDefinition, InMemoryCatalogProvider, Lookup, TableDefinition, UnknownReason,
    },
    diagnostic::Severity,
    resolution::{query, AnalysisStatus, Resolution, ResolutionUnknown},
    rules::{NoDistinct, NoUnknownReference},
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
        let prepared = query::extract(&tree.root_node());
        let acquired = InMemoryCatalogProvider::new(snapshot())
            .acquire(&prepared.requests)
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
            "SELECT DISTINCT id FROM users; SELECT missing FROM users",
            &cfg,
            Err(&error),
        )
        .unwrap();
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].code, "no-distinct");
    assert!(matches!(
        r.statements[0].status,
        AnalysisStatus::Excluded(_)
    ));
    assert_eq!(r.statements[1].status, AnalysisStatus::Failed(error));
}

#[test]
fn suppression_preserves_other_rules_lines_and_internal_resolution() {
    let mut cfg = config();
    cfg.rules
        .push((RuleEnum::NoDistinct(NoDistinct), Severity::Warning));
    let sql = "-- uroborosql-lint-disable-next-line no-unknown-reference\nSELECT , nmae FROM users;\nSELECT nmae FROM users; SELECT DISTINCT id FROM users;";
    let r = Linter::new()
        .run_with_catalog(sql, &cfg, Ok(&snapshot()))
        .unwrap();
    assert_eq!(
        r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
        ["no-unknown-reference", "no-distinct"]
    );
    assert_eq!(r.diagnostics[0].span.start.line, 2);
    assert!(r.statements[0].resolved.is_some());
    let sql = "-- uroborosql-lint-disable no-unknown-reference\nSELECT , nmae FROM users; SELECT DISTINCT id FROM users;";
    let r = Linter::new()
        .run_with_catalog(sql, &cfg, Ok(&snapshot()))
        .unwrap();
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].code, "no-distinct");
    assert!(matches!(
        r.statements[1].status,
        AnalysisStatus::Excluded(_)
    ));
    let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
    let r = Linter::new()
        .run_with_catalog(sql, &cfg, Err(&error))
        .unwrap();
    assert_eq!(r.statements[0].status, AnalysisStatus::Failed(error));
    assert!(matches!(
        r.statements[0].resolved.as_ref().unwrap().source,
        Resolution::Unknown(ResolutionUnknown::Unavailable(_))
    ));
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
    let result = run("SELECT /*param*/ AS result, id AS result, /*other*/ AS result FROM users;");
    let outputs = &result.statements[0].resolved.as_ref().unwrap().outputs;
    assert_eq!(
        outputs
            .iter()
            .map(|o| o.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("result"); 3]
    );
    assert_eq!(
        outputs
            .iter()
            .map(|o| o.references.len())
            .collect::<Vec<_>>(),
        [0, 1, 0]
    );
}

#[test]
fn recovered_sources_preserve_partial_results_without_absence_or_acquisition_failure() {
    for sql in [
        "SELECT x.id AS result, x AS result, wrong.nmae FROM /*#table*/ AS x WHERE x.id = 1;",
        "SELECT x.id AS result, x AS result, wrong.nmae FROM /*$table*/ AS x WHERE x.id = 1;",
        "SELECT , /*param*/ AS result, x AS result, wrong.nmae FROM , /*#table*/ AS x WHERE AND x.id = /*param*/ ;",
    ] {
        let tree = tree_sitter::parse_2way(sql).unwrap();
        assert!(query::extract(&tree.root_node()).requests.is_empty());
        let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
        for acquired in [Ok(&snapshot()), Err(&error)] {
            let result = Linter::new().run_with_catalog(sql, &config(), acquired).unwrap();
            assert!(result.diagnostics.is_empty(), "{sql}: {result:?}");
            let statement = &result.statements[0];
            assert_eq!(statement.status, AnalysisStatus::Complete);
            assert!(statement.exclusion.is_none());
            let resolved = statement.resolved.as_ref().unwrap();
            assert!(matches!(resolved.source, Resolution::Unknown(ResolutionUnknown::Reason(UnknownReason::RecoveredSource))));
            assert_eq!(resolved.outputs.iter().map(|o| o.name.as_deref()).collect::<Vec<_>>(), [Some("result"), Some("result"), None]);
            for reference in resolved.outputs.iter().flat_map(|o| &o.references).chain(&resolved.predicate_references) {
                assert!(matches!(reference.outcome, crate::resolution::ReferenceOutcome::Lookup { resolution: Resolution::Unknown(ResolutionUnknown::Reason(UnknownReason::RecoveredSource)), .. }));
                assert_eq!(&sql[reference.input.column.range.start_byte..reference.input.column.range.end_byte], reference.input.column.spelling);
            }
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
    let prepared = query::extract(&tree.root_node());
    let mut connection = PostgresConfig::new("127.0.0.1", "postgres", "postgres");
    connection.port = env::var("CATALOG_TEST_PORT")
        .expect("run tests/postgres/run.py")
        .parse()
        .expect("numeric fixture port");
    connection.password = Some(env::var("CATALOG_TEST_PASSWORD").expect("fixture password"));
    connection.tls_mode = TlsMode::Disable;
    let acquired = PostgresCatalogProvider::new(connection)
        .acquire(&prepared.requests)
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
