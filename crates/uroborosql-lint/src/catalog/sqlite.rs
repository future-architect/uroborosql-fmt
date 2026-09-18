//! Portable, validated catalog snapshots. Opening a provider is lazy and read-only.
use std::{collections::BTreeSet, path::PathBuf};

use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, Connection, SqliteConnection};

use super::{
    AbsenceKind, AcquisitionError, AcquisitionErrorKind, AcquisitionFuture, AcquisitionPhase,
    CatalogEntry, CatalogProvider, CatalogSnapshot, ColumnDefinition, Lookup, TableDefinition,
    TableRequest, UnknownReason,
};

mod data;
#[cfg(feature = "postgres-catalog")]
pub mod export;

use data::SnapshotData;

pub struct SqliteCatalogProvider {
    path: PathBuf,
}

impl SqliteCatalogProvider {
    /// Does not open the file. Callers resolve configuration-relative paths.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    async fn read(&self, requests: &[TableRequest]) -> Result<CatalogSnapshot, AcquisitionError> {
        let options = SqliteConnectOptions::new()
            .filename(&self.path)
            .read_only(true)
            .create_if_missing(false)
            .disable_statement_logging();
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .map_err(read_error)?;
        let mut transaction = connection.begin().await.map_err(read_error)?;
        let data = data::read_validated(&mut transaction).await?;
        let snapshot = data.resolve(requests)?;
        transaction.commit().await.map_err(read_error)?;
        connection.close().await.map_err(read_error)?;
        Ok(snapshot)
    }
}

impl CatalogProvider for SqliteCatalogProvider {
    fn acquire<'a>(&'a self, requests: &'a [TableRequest]) -> AcquisitionFuture<'a> {
        Box::pin(self.read(requests))
    }
}

impl SnapshotData {
    fn resolve(&self, requests: &[TableRequest]) -> Result<CatalogSnapshot, AcquisitionError> {
        let path: Vec<_> = self
            .path
            .values()
            .map(|oid| self.namespaces[oid].clone())
            .collect();
        let mut names = BTreeSet::new();
        for request in requests {
            match &request.schema {
                Some(schema) => {
                    names.insert((schema.clone(), request.name.clone()));
                }
                None => names.extend(
                    path.iter()
                        .map(|schema| (schema.clone(), request.name.clone())),
                ),
            }
        }
        CatalogSnapshot::new(
            path,
            names.into_iter().map(|(schema, table)| {
                let outcome = self.table(&schema, &table);
                CatalogEntry {
                    schema,
                    table,
                    outcome,
                }
            }),
        )
    }

    fn table(&self, schema: &str, name: &str) -> Lookup<TableDefinition> {
        if schema == "pg_temp" {
            return Lookup::Unknown(UnknownReason::UnsupportedSyntax);
        }
        let Some((&oid, _)) = self.namespaces.iter().find(|(_, n)| n.as_str() == schema) else {
            return Lookup::Absent(AbsenceKind::Schema);
        };
        if !self.access[&oid] {
            return Lookup::Unavailable(AcquisitionError::new(
                AcquisitionPhase::Schema,
                AcquisitionErrorKind::PermissionDenied,
            ));
        }
        let Some((&relation_oid, relation)) = self
            .relations
            .iter()
            .find(|(_, r)| r.namespace == oid && r.name == name)
        else {
            return Lookup::Absent(AbsenceKind::Table);
        };
        if relation.kind != "r" && relation.kind != "p" {
            return Lookup::Unknown(UnknownReason::UnsupportedRelation);
        }
        let mut columns = Vec::new();
        let mut system_columns = Vec::new();
        for ((_, number), attribute) in self
            .attributes
            .range((relation_oid, i16::MIN)..=(relation_oid, i16::MAX))
        {
            if attribute.dropped {
                continue;
            }
            let column = ColumnDefinition {
                name: attribute.name.clone(),
            };
            if *number > 0 {
                columns.push(column);
            } else {
                system_columns.push(column);
            }
        }
        Lookup::Found(TableDefinition {
            schema: schema.into(),
            name: name.into(),
            columns,
            system_columns,
        })
    }
}

fn invalid() -> AcquisitionError {
    AcquisitionError::new(
        AcquisitionPhase::Validate,
        AcquisitionErrorKind::InvalidData,
    )
}
fn read_error(_: sqlx::Error) -> AcquisitionError {
    invalid()
}
