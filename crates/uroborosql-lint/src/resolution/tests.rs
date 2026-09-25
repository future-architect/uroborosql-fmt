use super::*;
use crate::catalog::{
    AbsenceKind, AcquisitionErrorKind, AcquisitionPhase, CatalogEntry, ColumnDefinition,
};
use crate::resolution::query::extract;
use postgresql_cst_parser::tree_sitter;

fn table(schema: &str, name: &str, columns: &[&str]) -> TableDefinition {
    TableDefinition {
        schema: schema.into(),
        name: name.into(),
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
fn entry(schema: &str, name: &str, outcome: Lookup<TableDefinition>) -> CatalogEntry {
    CatalogEntry {
        schema: schema.into(),
        table: name.into(),
        outcome,
    }
}
fn snapshot() -> CatalogSnapshot {
    CatalogSnapshot::new(
        vec!["public".into()],
        [entry(
            "public",
            "users",
            Lookup::Found(table("public", "users", &["id", "name", "age"])),
        )],
    )
    .unwrap()
}
fn run(sql: &str, snapshot: &CatalogSnapshot) -> ResolvedSelect {
    resolve(
        &extract(&tree_sitter::parse_2way(sql).unwrap().root_node()),
        Ok(snapshot),
    )
    .pop()
    .unwrap()
    .resolved
    .unwrap()
}
fn column(reference: &Reference, name: &str) {
    assert!(
        matches!(&reference.outcome, ReferenceOutcome::Lookup { resolution: Resolution::Resolved(ResolvedValue::Column { name: actual, .. }), .. } if actual == name),
        "{reference:?}"
    );
}

#[test]
fn wildcard_projections_have_no_fabricated_columns_or_requests() {
    let sql = "SELECT x.*, missing, u.*, * FROM users u WHERE absent = 1";
    let prepared = extract(&tree_sitter::parse_2way(sql).unwrap().root_node());
    assert_eq!(prepared.requests.len(), 1);
    assert_eq!(prepared.requests[0].name, "users");
    let result = resolve(&prepared, Ok(&snapshot()));
    let select = result[0].resolved.as_ref().unwrap();
    assert_eq!(select.outputs.len(), 1);
    assert_eq!(select.projections.len(), 4);
    assert!(
        matches!(&select.projections[0], Projection::Wildcard { qualifier: Some((name, WildcardMatch::Mismatched)) } if name.spelling == "x" && &sql[name.range.start_byte..name.range.end_byte] == "x")
    );
    assert!(matches!(select.projections[1], Projection::Output(0)));
    assert!(matches!(
        &select.projections[2],
        Projection::Wildcard {
            qualifier: Some((_, WildcardMatch::Matched))
        }
    ));
    assert!(matches!(
        select.projections[3],
        Projection::Wildcard { qualifier: None }
    ));
    assert!(matches!(
        select.outputs[0].references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
    assert_eq!(select.predicate_references.len(), 1);
}

#[test]
fn wildcard_source_uncertainty_never_becomes_qualifier_mismatch() {
    let absent_snapshot = CatalogSnapshot::new(
        vec!["public".into()],
        [entry("public", "usres", Lookup::Absent(AbsenceKind::Table))],
    )
    .unwrap();
    let absent = run("SELECT x.* FROM usres u", &absent_snapshot);
    assert!(matches!(
        absent.source,
        Resolution::Absent(AbsenceKind::Table)
    ));
    assert!(matches!(
        absent.projections[0],
        Projection::Wildcard {
            qualifier: Some((_, WildcardMatch::Unknown))
        }
    ));
    let recovered = run("SELECT x.* FROM /*#table*/ AS u", &snapshot());
    assert!(matches!(recovered.source, Resolution::Unknown(_)));
    assert!(matches!(
        recovered.projections[0],
        Projection::Wildcard {
            qualifier: Some((_, WildcardMatch::Unknown))
        }
    ));
    let error = AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
    let prepared = extract(
        &tree_sitter::parse_2way("SELECT x.* FROM users u")
            .unwrap()
            .root_node(),
    );
    let failed = resolve(&prepared, Err(&error));
    let failed = failed[0].resolved.as_ref().unwrap();
    assert!(matches!(
        failed.projections[0],
        Projection::Wildcard {
            qualifier: Some((_, WildcardMatch::Unknown))
        }
    ));
}

#[test]
fn columns_aliases_and_system_columns_resolve_without_output_alias_visibility() {
    let r = run("SELECT u.id AS user_id, ctid, (age + 1) AS next_age FROM public.users u WHERE user_id = 1 AND u.age > 0", &snapshot());
    assert_eq!(
        r.outputs
            .iter()
            .map(|o| o.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("user_id"), Some("ctid"), Some("next_age")]
    );
    column(&r.outputs[0].references[0], "id");
    column(&r.outputs[1].references[0], "ctid");
    assert!(matches!(
        r.predicate_references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
    column(&r.predicate_references[1], "age");
    assert_eq!(r.predicate_references[1].clause, Clause::Where);
}

#[test]
fn whole_rows_are_one_output_and_real_columns_take_precedence() {
    let sql =
        "SELECT u, u AS row_value, u FROM users u WHERE u IS NOT NULL AND row_value IS NOT NULL";
    let r = run(sql, &snapshot());
    assert_eq!(
        r.outputs
            .iter()
            .map(|o| o.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("u"), Some("row_value"), Some("u")]
    );
    for output in &r.outputs {
        assert!(matches!(
            output.references[0].outcome,
            ReferenceOutcome::Lookup {
                resolution: Resolution::Resolved(ResolvedValue::WholeRow(_)),
                ..
            }
        ));
    }
    assert!(matches!(
        r.predicate_references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Resolved(ResolvedValue::WholeRow(_)),
            ..
        }
    ));
    assert!(matches!(
        r.predicate_references[1].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
    let with_column = CatalogSnapshot::new(
        vec!["public".into()],
        [entry(
            "public",
            "users",
            Lookup::Found(table("public", "users", &["u"])),
        )],
    )
    .unwrap();
    let r = run(sql, &with_column);
    column(&r.outputs[0].references[0], "u");
}

#[test]
fn hidden_original_name_and_unknown_qualifier_are_distinct() {
    let r = run(
        "SELECT users, users.id, x.missing, u.missing FROM users AS u",
        &snapshot(),
    );
    assert!(matches!(
        r.outputs[0].references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
    assert!(matches!(
        r.outputs[1].references[0].outcome,
        ReferenceOutcome::QualifierMismatch { .. }
    ));
    assert!(matches!(
        r.outputs[2].references[0].outcome,
        ReferenceOutcome::QualifierMismatch { .. }
    ));
    assert!(matches!(
        r.outputs[3].references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
}

#[test]
fn source_search_does_not_fill_columns_from_later_tables() {
    let definitions = vec![
        entry(
            "app",
            "users",
            Lookup::Found(table("app", "users", &["id"])),
        ),
        entry(
            "public",
            "users",
            Lookup::Found(table("public", "users", &["name"])),
        ),
    ];
    let s = CatalogSnapshot::new(vec!["app".into(), "public".into()], definitions).unwrap();
    let r = run("SELECT name FROM users", &s);
    assert!(matches!(
        r.outputs[0].references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
    column(
        &run("SELECT name FROM public.users", &s).outputs[0].references[0],
        "name",
    );
    for first in [
        Lookup::Absent(AbsenceKind::Table),
        Lookup::Unknown(UnknownReason::UnsupportedRelation),
    ] {
        let falls_through = matches!(first, Lookup::Absent(_));
        let s = CatalogSnapshot::new(
            vec!["pg_catalog".into(), "public".into()],
            [
                entry("pg_catalog", "users", first),
                entry(
                    "public",
                    "users",
                    Lookup::Found(table("public", "users", &["name"])),
                ),
            ],
        )
        .unwrap();
        let r = run("SELECT name FROM users", &s);
        if falls_through {
            column(&r.outputs[0].references[0], "name");
        } else {
            assert!(matches!(
                r.source,
                Resolution::Unknown(ResolutionUnknown::Reason(
                    UnknownReason::UnsupportedRelation
                ))
            ));
        }
    }
}

#[test]
fn absent_unknown_and_unavailable_sources_propagate_without_qualifier_errors() {
    let error = AcquisitionError::new(
        AcquisitionPhase::Schema,
        AcquisitionErrorKind::PermissionDenied,
    );
    for outcome in [
        Lookup::Absent(AbsenceKind::Schema),
        Lookup::Absent(AbsenceKind::Table),
        Lookup::Unknown(UnknownReason::UnsupportedRelation),
        Lookup::Unavailable(error.clone()),
    ] {
        let s = CatalogSnapshot::new(vec!["public".into()], [entry("public", "users", outcome)])
            .unwrap();
        let r = run("SELECT x.id, u FROM users u WHERE absent > 1", &s);
        assert!(r
            .outputs
            .iter()
            .flat_map(|o| &o.references)
            .chain(&r.predicate_references)
            .all(|r| matches!(
                r.outcome,
                ReferenceOutcome::Lookup {
                    resolution: Resolution::Absent(_) | Resolution::Unknown(_),
                    ..
                }
            )));
    }
    let s = CatalogSnapshot::new(vec!["public".into()], []).unwrap();
    assert!(matches!(
        run("SELECT id FROM users", &s).source,
        Resolution::Unknown(ResolutionUnknown::Reason(UnknownReason::IncompleteCoverage))
    ));
    let prepared = extract(
        &tree_sitter::parse_2way("SELECT id FROM users; SELECT DISTINCT ON (id) id FROM users")
            .unwrap()
            .root_node(),
    );
    let r = resolve(&prepared, Err(&error));
    assert_eq!(r[0].status, AnalysisStatus::Failed(error));
    assert!(matches!(r[1].status, AnalysisStatus::Excluded(_)));
    assert!(r[1].exclusion.is_some());
}

#[test]
fn quoted_spelling_and_zero_column_tables_are_preserved() {
    let s = CatalogSnapshot::new(
        vec!["public".into()],
        [
            entry(
                "public",
                "Users",
                Lookup::Found(table("public", "Users", &["Id"])),
            ),
            entry(
                "public",
                "empty",
                Lookup::Found(table("public", "empty", &[])),
            ),
        ],
    )
    .unwrap();
    let r = run(
        "SELECT \"U\".\"Id\", id, u.\"Id\", \"U\" FROM public.\"Users\" AS \"U\"",
        &s,
    );
    column(&r.outputs[0].references[0], "Id");
    assert!(matches!(
        r.outputs[1].references[0].outcome,
        ReferenceOutcome::Lookup {
            resolution: Resolution::Absent(AbsenceKind::Column),
            ..
        }
    ));
    assert!(matches!(
        r.outputs[2].references[0].outcome,
        ReferenceOutcome::QualifierMismatch { .. }
    ));
    let r = run("SELECT empty, ctid, id, 1, id + 1 FROM empty", &s);
    assert!(matches!(r.source, Resolution::Resolved(_)));
    assert_eq!(
        r.outputs
            .iter()
            .map(|o| o.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("empty"), Some("ctid"), None, None, None]
    );
}

#[test]
fn implicit_output_names_require_a_direct_reference_through_groups() {
    let r = run(
        "SELECT ((id)), +id, id + 1, id IS NULL, (u), (+id) AS named FROM users u",
        &snapshot(),
    );
    assert_eq!(
        r.outputs
            .iter()
            .map(|o| o.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("id"), None, None, None, Some("u"), Some("named")]
    );
    assert!(r.outputs.iter().all(|o| o.references.len() == 1));
}
