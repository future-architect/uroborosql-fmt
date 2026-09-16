use std::task::{Context, Poll, Waker};
use uroborosql_lint::catalog::*;

fn entry(schema: &str, outcome: Lookup<TableDefinition>) -> CatalogEntry {
    CatalogEntry {
        schema: schema.into(),
        table: "users".into(),
        outcome,
    }
}
fn table(schema: &str, columns: &[&str]) -> TableDefinition {
    TableDefinition {
        schema: schema.into(),
        name: "users".into(),
        columns: columns
            .iter()
            .map(|name| ColumnDefinition {
                name: (*name).into(),
            })
            .collect(),
        system_columns: vec![ColumnDefinition {
            name: "ctid".into(),
        }],
    }
}
fn request(schema: Option<&str>) -> TableRequest {
    TableRequest {
        schema: schema.map(str::to_owned),
        name: "users".into(),
    }
}
fn snapshot(first: Lookup<TableDefinition>) -> CatalogSnapshot {
    CatalogSnapshot::new(
        vec!["app".into(), "public".into()],
        [
            entry("app", first),
            entry("public", Lookup::Found(table("public", &["id", "name"]))),
        ],
    )
    .unwrap()
}
fn failure() -> AcquisitionError {
    AcquisitionError {
        phase: AcquisitionPhase::Schema,
        kind: AcquisitionErrorKind::PermissionDenied,
    }
}

#[test]
fn first_relation_wins_without_column_fallback() {
    let data = snapshot(Lookup::Found(table("app", &["Id"])));
    assert_eq!(data.effective_search_path(), &["app", "public"]);
    let Lookup::Found(found) = data.lookup(&request(None)) else {
        panic!("expected app")
    };
    assert_eq!(found.schema, "app");
    assert_eq!(found.column("id"), Lookup::Absent(AbsenceKind::Column));
    assert!(matches!(found.column("Id"), Lookup::Found(_)));
    assert!(matches!(found.column("ctid"), Lookup::Found(_)));
    let Lookup::Found(explicit) = data.lookup(&request(Some("public"))) else {
        panic!("expected public")
    };
    assert_eq!(
        explicit
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "name"]
    );
}

#[test]
fn only_confirmed_absence_falls_through() {
    for absence in [AbsenceKind::Schema, AbsenceKind::Table] {
        let data = snapshot(Lookup::Absent(absence));
        assert!(matches!(data.lookup(&request(None)), Lookup::Found(t) if t.schema == "public"));
        assert_eq!(data.lookup(&request(Some("app"))), Lookup::Absent(absence));
    }
    for unknown in [
        UnknownReason::UnsupportedRelation,
        UnknownReason::IncompleteCoverage,
    ] {
        assert_eq!(
            snapshot(Lookup::Unknown(unknown)).lookup(&request(None)),
            Lookup::Unknown(unknown)
        );
    }
    assert_eq!(
        snapshot(Lookup::Unavailable(failure())).lookup(&request(None)),
        Lookup::Unavailable(failure())
    );
}

#[test]
fn missing_acquisition_is_unknown_and_complete_absence_is_explicit() {
    let data = CatalogSnapshot::new(
        vec!["pg_catalog".into(), "public".into()],
        [entry("public", Lookup::Found(table("public", &[])))],
    )
    .unwrap();
    assert_eq!(
        data.lookup(&request(None)),
        Lookup::Unknown(UnknownReason::IncompleteCoverage)
    );
    assert_eq!(
        data.lookup(&request(Some("missing"))),
        Lookup::Unknown(UnknownReason::IncompleteCoverage)
    );
    let absent = CatalogSnapshot::new(
        vec!["public".into()],
        [entry("public", Lookup::Absent(AbsenceKind::Table))],
    )
    .unwrap();
    assert_eq!(
        absent.lookup(&request(None)),
        Lookup::Absent(AbsenceKind::Table)
    );
    let empty_path = CatalogSnapshot::new(vec![], []).unwrap();
    assert_eq!(
        empty_path.lookup(&request(None)),
        Lookup::Absent(AbsenceKind::Table)
    );
    let Lookup::Found(zero) = data.lookup(&request(Some("public"))) else {
        panic!("zero columns still exists")
    };
    assert!(zero.columns.is_empty());
    assert_eq!(zero.column("id"), Lookup::Absent(AbsenceKind::Column));
}

#[test]
fn invalid_definitions_never_become_absence() {
    let duplicate = entry("app", Lookup::Absent(AbsenceKind::Table));
    let invalid = [
        vec![duplicate.clone(), duplicate],
        vec![entry("app", Lookup::Found(table("public", &["id"])))],
        vec![entry("app", Lookup::Found(table("app", &["id", "id"])))],
        vec![entry("app", Lookup::Found(table("app", &["ctid"])))],
        vec![entry("app", Lookup::Absent(AbsenceKind::Column))],
    ];
    for entries in invalid {
        assert_eq!(
            CatalogSnapshot::new(vec![], entries).unwrap_err(),
            AcquisitionError {
                phase: AcquisitionPhase::Validate,
                kind: AcquisitionErrorKind::InvalidData
            }
        );
    }
}

fn acquire(provider: &dyn CatalogProvider) -> Result<CatalogSnapshot, AcquisitionError> {
    let requests = [request(None)];
    let mut future = provider.acquire(&requests);
    fn assert_send<T: Send>(_: &T) {}
    assert_send(&future);
    // A pending result fails this test, so no task needs to be scheduled again.
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("memory provider must be ready"),
    }
}

#[test]
fn memory_provider_supports_dynamic_dispatch_send_future_and_sync_consumption() {
    let expected = snapshot(Lookup::Found(table("app", &["id"])));
    let provider = InMemoryCatalogProvider::new(expected.clone());
    assert_eq!(acquire(&provider).unwrap(), expected);
    assert_eq!(
        acquire(&InMemoryCatalogProvider::failing(failure())),
        Err(failure())
    );
    assert_eq!(
        Resolution::<()>::from(Lookup::Unavailable(failure())),
        Resolution::Unknown(ResolutionUnknown::Unavailable(failure()))
    );
    assert_eq!(
        Resolution::<()>::from(Lookup::Unknown(UnknownReason::UnsupportedRelation)),
        Resolution::Unknown(ResolutionUnknown::Reason(
            UnknownReason::UnsupportedRelation
        ))
    );
    assert_ne!(
        Resolution::<()>::Ambiguous,
        Resolution::Absent(AbsenceKind::Column)
    );
}
