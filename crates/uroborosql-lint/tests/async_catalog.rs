use std::sync::Mutex;
use uroborosql_lint::{
    catalog::{
        AcquisitionError, AcquisitionErrorKind, AcquisitionFuture, AcquisitionPhase,
        AnalysisStatus, CatalogEntry, CatalogProvider, CatalogSnapshot, ColumnDefinition,
        InMemoryCatalogProvider, Lookup, TableDefinition, TableRequest,
    },
    CatalogReport, CatalogSkipReason, ConfigStore, Linter, ResolvedLintConfig,
};

struct RecordingProvider {
    calls: Mutex<Vec<Vec<TableRequest>>>,
    result: Result<CatalogSnapshot, AcquisitionError>,
}
impl CatalogProvider for RecordingProvider {
    fn acquire<'a>(&'a self, requests: &'a [TableRequest]) -> AcquisitionFuture<'a> {
        self.calls.lock().unwrap().push(requests.to_vec());
        Box::pin(std::future::ready(self.result.clone()))
    }
}
fn snapshot() -> CatalogSnapshot {
    CatalogSnapshot::new(
        vec!["public".into()],
        vec![CatalogEntry {
            schema: "public".into(),
            table: "users".into(),
            outcome: Lookup::Found(TableDefinition {
                schema: "public".into(),
                name: "users".into(),
                columns: vec![ColumnDefinition { name: "id".into() }],
                system_columns: vec![],
            }),
        }],
    )
    .unwrap()
}
fn config(json: &str) -> ResolvedLintConfig {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".uroborosqllintrc.json"), json).unwrap();
    ConfigStore::try_new(dir.path(), None)
        .unwrap()
        .unwrap()
        .resolve(&dir.path().join("query.sql"))
}
fn assert_send<T: Send>(_: &T) {}

#[tokio::test]
async fn public_async_is_send_deduplicates_and_preserves_sync_behavior() {
    let provider = RecordingProvider {
        calls: Mutex::new(vec![]),
        result: Ok(snapshot()),
    };
    let cfg = ResolvedLintConfig::default();
    let linter = Linter::new();
    let sql = "SELECT missing FROM users; SELECT missing FROM users;";
    let future = linter.run_async(sql, &cfg, Some(&provider));
    assert_send(&future);
    let result = future.await.unwrap();
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "no-unknown-reference")
            .count(),
        2
    );
    assert!(linter
        .run(sql, &cfg)
        .unwrap()
        .iter()
        .all(|d| d.code != "no-unknown-reference"));
    assert_eq!(
        *provider.calls.lock().unwrap(),
        vec![vec![TableRequest {
            schema: None,
            name: "users".into()
        }]]
    );
    let CatalogReport::Statements(statements) = result.catalog else {
        panic!()
    };
    assert!(statements
        .iter()
        .all(|s| s.status == AnalysisStatus::Complete));
    assert_eq!(
        &sql[statements[0].span.start.byte..statements[0].span.end.byte],
        "SELECT missing FROM users"
    );
}

#[tokio::test]
async fn no_acquisition_for_skipped_empty_unsupported_recovered_or_parse_failure() {
    let provider = RecordingProvider {
        calls: Mutex::new(vec![]),
        result: Err(AcquisitionError::new(
            AcquisitionPhase::Connect,
            AcquisitionErrorKind::Connection,
        )),
    };
    let linter = Linter::new();
    let default = ResolvedLintConfig::default();
    assert!(matches!(
        linter
            .run_async("SELECT id FROM users", &default, None)
            .await
            .unwrap()
            .catalog,
        CatalogReport::Skipped(CatalogSkipReason::NotConfigured)
    ));
    let off = config(r#"{"rules":{"no-unknown-reference":"off"}}"#);
    assert!(matches!(
        linter
            .run_async("SELECT id FROM users", &off, Some(&provider))
            .await
            .unwrap()
            .catalog,
        CatalogReport::Skipped(CatalogSkipReason::RuleDisabled)
    ));
    let overridden =
        config(r#"{"overrides":[{"files":["query.sql"],"rules":{"no-unknown-reference":"off"}}]}"#);
    linter
        .run_async("SELECT id FROM users", &overridden, Some(&provider))
        .await
        .unwrap();
    for sql in [
        "",
        "SELECT 1",
        "SET search_path TO public; SELECT id FROM users",
        "SELECT x.id FROM /*#table*/ AS x",
    ] {
        let result = linter
            .run_async(sql, &default, Some(&provider))
            .await
            .unwrap();
        assert!(!result.catalog.has_failures(), "{sql}");
        if sql.contains("#table") {
            let CatalogReport::Statements(statements) = result.catalog else {
                panic!()
            };
            assert!(statements[0].recovered_source);
            assert_eq!(statements[0].status, AnalysisStatus::Complete);
        }
    }
    assert!(linter
        .run_async("SELECT 1 + ;", &default, Some(&provider))
        .await
        .is_err());
    assert!(provider.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failure_retains_cst_and_exclusions_with_one_suppression_pass() {
    let provider = InMemoryCatalogProvider::failing(AcquisitionError::new(
        AcquisitionPhase::Connect,
        AcquisitionErrorKind::Connection,
    ));
    let sql = "-- uroborosql-lint-disable no-distinct, not-a-rule\nSELECT DISTINCT ON (id) id FROM users; SELECT missing FROM users; SELECT x.id FROM /*#table*/ AS x;";
    let result = Linter::new()
        .run_async(sql, &ResolvedLintConfig::default(), Some(&provider))
        .await
        .unwrap();
    assert!(result.catalog.has_failures());
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|d| d.code == "invalid-lint-directive")
            .count(),
        1
    );
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code != "no-distinct" && d.code != "no-unknown-reference"));
    let CatalogReport::Statements(statements) = result.catalog else {
        panic!()
    };
    assert!(matches!(statements[0].status, AnalysisStatus::Excluded(_)));
    assert!(matches!(statements[1].status, AnalysisStatus::Failed(_)));
    assert_eq!(statements[2].status, AnalysisStatus::Complete);
    let retained = Linter::new()
        .run_async(
            "SELECT DISTINCT id FROM users; SELECT id FROM users;",
            &ResolvedLintConfig::default(),
            Some(&provider),
        )
        .await
        .unwrap();
    assert!(retained.diagnostics.iter().any(|d| d.code == "no-distinct"));
}

#[tokio::test]
async fn catalog_diagnostics_obey_directives_severity_and_scoped_unavailable() {
    let provider = InMemoryCatalogProvider::new(snapshot());
    let cfg = config(r#"{"rules":{"no-unknown-reference":"warn"}}"#);
    let sql = "-- uroborosql-lint-disable-next-line no-unknown-reference\nSELECT missing FROM users;\nSELECT missing FROM users;";
    let result = Linter::new()
        .run_async(sql, &cfg, Some(&provider))
        .await
        .unwrap();
    let diagnostics: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "no-unknown-reference")
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].span.start.line, 2);
    assert_eq!(diagnostics[0].severity, uroborosql_lint::Severity::Warning);
    let provider = InMemoryCatalogProvider::new(
        CatalogSnapshot::new(
            vec!["public".into()],
            vec![CatalogEntry {
                schema: "public".into(),
                table: "users".into(),
                outcome: Lookup::Unavailable(AcquisitionError::new(
                    AcquisitionPhase::Schema,
                    AcquisitionErrorKind::PermissionDenied,
                )),
            }],
        )
        .unwrap(),
    );
    let result = Linter::new()
        .run_async("SELECT missing FROM users", &cfg, Some(&provider))
        .await
        .unwrap();
    assert!(result.catalog.has_failures());
    assert!(result
        .diagnostics
        .iter()
        .all(|d| d.code != "no-unknown-reference"));
}

#[tokio::test]
async fn configured_unavailable_provider_is_lazy_and_keeps_diagnostics() {
    let cfg = config(r#"{"db":{"schemaProvider":"file","path":"/does-not-exist/catalog.sqlite"}}"#);
    let provider = cfg.catalog_provider().unwrap();
    let result = Linter::new()
        .run_async(
            "SELECT DISTINCT id FROM users; SELECT id FROM users;",
            &cfg,
            Some(provider.as_ref()),
        )
        .await
        .unwrap();
    assert!(result.catalog.has_failures());
    assert!(result.diagnostics.iter().any(|d| d.code == "no-distinct"));
    let CatalogReport::Statements(statements) = result.catalog else {
        panic!()
    };
    let AnalysisStatus::Failed(error) = &statements[1].status else {
        panic!()
    };
    assert!(error.to_string().contains("File catalog is unavailable"));
}

#[cfg(feature = "postgres-catalog")]
#[tokio::test]
async fn configured_timeouts_are_validated_only_when_acquisition_is_needed() {
    use uroborosql_lint::catalog::{AcquisitionDetail, ConfigurationField};
    for (key, field) in [
        ("connectMs", ConfigurationField::ConnectTimeout),
        ("queryMs", ConfigurationField::QueryTimeout),
        ("acquisitionMs", ConfigurationField::AcquisitionTimeout),
    ] {
        let cfg = config(&format!(
            r#"{{"db":{{"schemaProvider":"server","host":"localhost","user":"private-user","password":"private-password","dbname":"app","tlsMode":"disable","timeouts":{{"{key}":0}}}}}}"#
        ));
        let provider = cfg.catalog_provider().unwrap();
        let no_requests = Linter::new()
            .run_async("SELECT 1", &cfg, Some(provider.as_ref()))
            .await
            .unwrap();
        assert!(!no_requests.catalog.has_failures());
        let result = Linter::new()
            .run_async("SELECT id FROM users", &cfg, Some(provider.as_ref()))
            .await
            .unwrap();
        let CatalogReport::Statements(statements) = result.catalog else {
            panic!()
        };
        let AnalysisStatus::Failed(error) = &statements[0].status else {
            panic!()
        };
        assert_eq!(
            error.detail,
            Some(AcquisitionDetail::InvalidConfiguration(field))
        );
        assert!(!format!("{error} {error:?}").contains("private-"));
    }
}

#[cfg(not(feature = "postgres-catalog"))]
#[tokio::test]
async fn server_config_without_feature_is_an_explicit_acquisition_failure() {
    let cfg = config(
        r#"{"db":{"schemaProvider":"server","host":"localhost","user":"user","dbname":"app"}}"#,
    );
    let provider = cfg.catalog_provider().unwrap();
    let result = Linter::new()
        .run_async("SELECT id FROM users", &cfg, Some(provider.as_ref()))
        .await
        .unwrap();
    assert!(result.catalog.has_failures());
    let CatalogReport::Statements(statements) = result.catalog else {
        panic!()
    };
    let AnalysisStatus::Failed(error) = &statements[0].status else {
        panic!()
    };
    assert!(error.to_string().contains("postgres-catalog"));
}
