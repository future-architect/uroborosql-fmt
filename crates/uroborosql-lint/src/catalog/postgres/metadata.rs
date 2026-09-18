use std::collections::BTreeSet;

use sqlx::{PgConnection, Row};

use super::super::{
    AbsenceKind, AcquisitionDetail, CatalogEntry, ColumnDefinition, Lookup, TableDefinition,
    UnknownReason,
};
use super::{
    error, query, AcquisitionError, AcquisitionErrorKind, AcquisitionPhase, CatalogSnapshot,
    TableRequest,
};

pub(super) async fn read_snapshot(
    connection: &mut PgConnection,
    requests: &[TableRequest],
    phase: &mut AcquisitionPhase,
) -> Result<CatalogSnapshot, AcquisitionError> {
    query(
        phase,
        AcquisitionPhase::SearchPath,
        sqlx::query("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *connection),
    )
    .await?;
    let (version, search_path): (String, Vec<String>) = query(
        phase,
        AcquisitionPhase::SearchPath,
        sqlx::query_as(
            "SELECT pg_catalog.current_setting('server_version_num'), pg_catalog.current_schemas(true)::text[]",
        )
        .fetch_one(&mut *connection),
    )
    .await?;
    validate_server_version(&version)?;
    let mut names = BTreeSet::new();
    for request in requests {
        match &request.schema {
            Some(schema) => {
                names.insert((schema.clone(), request.name.clone()));
            }
            None => {
                names.extend(
                    search_path
                        .iter()
                        .map(|schema| (schema.clone(), request.name.clone())),
                );
            }
        }
    }
    let mut entries = Vec::with_capacity(names.len());
    for (schema, table) in names {
        let outcome = if schema == "pg_temp" {
            Lookup::Unknown(UnknownReason::UnsupportedSyntax)
        } else {
            read_table(connection, &schema, &table, phase).await?
        };
        entries.push(CatalogEntry {
            schema,
            table,
            outcome,
        });
    }
    let snapshot = CatalogSnapshot::new(search_path, entries)?;
    query(
        phase,
        AcquisitionPhase::Validate,
        sqlx::query("COMMIT").execute(&mut *connection),
    )
    .await?;
    Ok(snapshot)
}

fn validate_server_version(version: &str) -> Result<(), AcquisitionError> {
    let major = version.parse::<u32>().map_err(|_| {
        error(
            AcquisitionPhase::Validate,
            AcquisitionErrorKind::InvalidData,
        )
    })? / 10_000;
    if !(14..=18).contains(&major) {
        return Err(error(
            AcquisitionPhase::Validate,
            AcquisitionErrorKind::InvalidData,
        )
        .with_detail(AcquisitionDetail::UnsupportedServerVersion));
    }
    Ok(())
}

async fn read_table(
    connection: &mut PgConnection,
    schema: &str,
    table: &str,
    phase: &mut AcquisitionPhase,
) -> Result<Lookup<TableDefinition>, AcquisitionError> {
    let row = query(
        phase,
        AcquisitionPhase::Relation,
        sqlx::query(
            "SELECT pg_catalog.has_schema_privilege(n.oid, 'USAGE') AS usable,
                    c.oid, c.relkind::text AS kind, c.relnatts
             FROM pg_catalog.pg_namespace n
             LEFT JOIN pg_catalog.pg_class c ON c.relnamespace = n.oid AND c.relname::text = $2
             WHERE n.nspname::text = $1",
        )
        .bind(schema)
        .bind(table)
        .fetch_optional(&mut *connection),
    )
    .await?;
    let Some(row) = row else {
        return Ok(Lookup::Absent(AbsenceKind::Schema));
    };
    if !row.try_get::<bool, _>("usable").map_err(invalid_row)? {
        return Ok(Lookup::Unavailable(error(
            AcquisitionPhase::Schema,
            AcquisitionErrorKind::PermissionDenied,
        )));
    }
    let Some(oid) = row
        .try_get::<Option<sqlx::postgres::types::Oid>, _>("oid")
        .map_err(invalid_row)?
    else {
        return Ok(Lookup::Absent(AbsenceKind::Table));
    };
    let kind: String = row.try_get("kind").map_err(invalid_row)?;
    if kind != "r" && kind != "p" {
        return Ok(Lookup::Unknown(UnknownReason::UnsupportedRelation));
    }
    let relnatts: i16 = row.try_get("relnatts").map_err(invalid_row)?;
    let rows = query(
        phase,
        AcquisitionPhase::Columns,
        sqlx::query(
            "SELECT c.relnatts, a.attnum, a.attname::text AS name, a.attisdropped
             FROM pg_catalog.pg_class c
             LEFT JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid
             WHERE c.oid = $1 ORDER BY a.attnum",
        )
        .bind(oid)
        .fetch_all(&mut *connection),
    )
    .await?;
    if rows.is_empty() || relnatts < 0 {
        return Err(error(
            AcquisitionPhase::Columns,
            AcquisitionErrorKind::InvalidData,
        ));
    }
    let mut attributes = Vec::new();
    for row in rows {
        if row.try_get::<i16, _>("relnatts").map_err(invalid_row)? != relnatts {
            return Err(error(
                AcquisitionPhase::Columns,
                AcquisitionErrorKind::InvalidData,
            ));
        }
        if let Some(number) = row
            .try_get::<Option<i16>, _>("attnum")
            .map_err(invalid_row)?
        {
            attributes.push(Attribute {
                number,
                name: row.try_get("name").map_err(invalid_row)?,
                dropped: row.try_get("attisdropped").map_err(invalid_row)?,
            });
        }
    }
    let (columns, system_columns) = validate_attributes(relnatts, attributes)?;
    Ok(Lookup::Found(TableDefinition {
        schema: schema.to_owned(),
        name: table.to_owned(),
        columns,
        system_columns,
    }))
}

struct Attribute {
    number: i16,
    name: String,
    dropped: bool,
}

fn validate_attributes(
    relnatts: i16,
    attributes: Vec<Attribute>,
) -> Result<(Vec<ColumnDefinition>, Vec<ColumnDefinition>), AcquisitionError> {
    let invalid = || error(AcquisitionPhase::Columns, AcquisitionErrorKind::InvalidData);
    let mut numbers = BTreeSet::new();
    let mut columns = Vec::new();
    let mut system_columns = Vec::new();
    for attribute in attributes {
        if attribute.number == 0
            || attribute.number > relnatts
            || !numbers.insert(attribute.number)
            || (attribute.number < 0 && attribute.dropped)
        {
            return Err(invalid());
        }
        if !attribute.dropped {
            let column = ColumnDefinition {
                name: attribute.name,
            };
            if attribute.number > 0 {
                columns.push(column);
            } else {
                system_columns.push(column);
            }
        }
    }
    if !(1..=relnatts).all(|number| numbers.contains(&number)) {
        return Err(invalid());
    }
    Ok((columns, system_columns))
}

fn invalid_row(_: sqlx::Error) -> AcquisitionError {
    error(AcquisitionPhase::Columns, AcquisitionErrorKind::InvalidData)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attribute(number: i16, name: &str, dropped: bool) -> Attribute {
        Attribute {
            number,
            name: name.into(),
            dropped,
        }
    }

    #[test]
    fn unsupported_versions_explain_the_supported_range() {
        for version in ["130023", "190000"] {
            let error = validate_server_version(version).unwrap_err();
            assert_eq!(
                error.detail,
                Some(AcquisitionDetail::UnsupportedServerVersion)
            );
            assert!(error.to_string().contains("PostgreSQL 14 through 18"));
        }
        assert!(validate_server_version("140000").is_ok());
        assert!(validate_server_version("180006").is_ok());
        let malformed = validate_server_version("private-invalid-value").unwrap_err();
        assert_eq!(malformed.detail, None);
        assert!(!format!("{malformed} {malformed:?}").contains("private-invalid-value"));
    }

    #[test]
    fn incomplete_or_inconsistent_attributes_fail() {
        for attributes in [
            vec![attribute(1, "one", false)],
            vec![attribute(1, "one", false), attribute(1, "duplicate", false)],
            vec![attribute(0, "invalid", false), attribute(2, "two", false)],
            vec![
                attribute(1, "one", false),
                attribute(3, "out_of_range", false),
            ],
            vec![
                attribute(-1, "ctid", true),
                attribute(1, "one", false),
                attribute(2, "two", false),
            ],
        ] {
            assert_eq!(
                validate_attributes(2, attributes).unwrap_err().kind,
                AcquisitionErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn dropped_slots_count_toward_completeness() {
        let (columns, system) = validate_attributes(
            3,
            vec![
                attribute(-1, "ctid", false),
                attribute(1, "one", false),
                attribute(2, "dropped", true),
                attribute(3, "three", false),
            ],
        )
        .unwrap();
        assert_eq!(
            columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["one", "three"]
        );
        assert_eq!(system[0].name, "ctid");
    }
}
