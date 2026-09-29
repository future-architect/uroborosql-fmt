//! Callers normalize lookup names; providers preserve catalog spelling and
//! certify completeness before claiming absence.
use std::{collections::BTreeMap, future::Future, pin::Pin};

// Preserve the existing public type paths while SQL-local results live in resolution.
pub use crate::resolution::{AnalysisStatus, Resolution, ResolutionUnknown};

#[cfg(feature = "postgres-catalog")]
pub mod postgres;

mod error;
pub use error::{
    AcquisitionDetail, AcquisitionError, AcquisitionErrorKind, AcquisitionPhase,
    ConfigurationField, TimeoutScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDefinition {
    pub name: String,
}

/// A complete supported relation definition, including zero-column relations.
/// Dropped columns are omitted; user columns retain catalog order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDefinition {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnDefinition>,
    pub system_columns: Vec<ColumnDefinition>,
}

impl TableDefinition {
    pub fn column(&self, name: &str) -> Lookup<&ColumnDefinition> {
        self.columns
            .iter()
            .chain(&self.system_columns)
            .find(|column| column.name == name)
            .map_or(Lookup::Absent(AbsenceKind::Column), Lookup::Found)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRequest {
    pub schema: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsenceKind {
    Schema,
    Table,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    RecoveredSource,
    UnsupportedRelation,
    IncompleteCoverage,
    UnsupportedSyntax,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup<T> {
    Found(T),
    Absent(AbsenceKind),
    Unknown(UnknownReason),
    Unavailable(AcquisitionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub schema: String,
    pub table: String,
    pub outcome: Lookup<TableDefinition>,
}

/// Valid for one analysis; reuse across analyses requires separate invalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSnapshot {
    effective_search_path: Vec<String>,
    tables: BTreeMap<(String, String), Lookup<TableDefinition>>,
}

impl CatalogSnapshot {
    /// Providers must certify absence and completeness; structural validation
    /// here cannot detect rows omitted by the source.
    pub fn new(
        effective_search_path: Vec<String>,
        entries: impl IntoIterator<Item = CatalogEntry>,
    ) -> Result<Self, AcquisitionError> {
        let invalid = || {
            AcquisitionError::new(
                AcquisitionPhase::Validate,
                AcquisitionErrorKind::InvalidData,
            )
        };
        let mut tables = BTreeMap::new();
        for entry in entries {
            if let Lookup::Found(definition) = &entry.outcome {
                if definition.schema != entry.schema || definition.name != entry.table {
                    return Err(invalid());
                }
                let mut names = std::collections::BTreeSet::new();
                if !definition
                    .columns
                    .iter()
                    .chain(&definition.system_columns)
                    .all(|column| names.insert(&column.name))
                {
                    return Err(invalid());
                }
            }
            if matches!(entry.outcome, Lookup::Absent(AbsenceKind::Column))
                || tables
                    .insert((entry.schema, entry.table), entry.outcome)
                    .is_some()
            {
                return Err(invalid());
            }
        }
        Ok(Self {
            effective_search_path,
            tables,
        })
    }

    pub fn effective_search_path(&self) -> &[String] {
        &self.effective_search_path
    }

    pub fn lookup_qualified(&self, schema: &str, table: &str) -> Lookup<&TableDefinition> {
        match self.tables.get(&(schema.to_owned(), table.to_owned())) {
            Some(Lookup::Found(table)) => Lookup::Found(table),
            Some(Lookup::Absent(reason)) => Lookup::Absent(*reason),
            Some(Lookup::Unknown(reason)) => Lookup::Unknown(*reason),
            Some(Lookup::Unavailable(error)) => Lookup::Unavailable(error.clone()),
            None => Lookup::Unknown(UnknownReason::IncompleteCoverage),
        }
    }

    /// Falling through on unknown/unavailable could bind a different relation
    /// in a later schema and produce false diagnostics.
    pub fn lookup(&self, request: &TableRequest) -> Lookup<&TableDefinition> {
        if let Some(schema) = &request.schema {
            return self.lookup_qualified(schema, &request.name);
        }
        for schema in &self.effective_search_path {
            match self.lookup_qualified(schema, &request.name) {
                Lookup::Absent(_) => continue,
                result => return result,
            }
        }
        Lookup::Absent(AbsenceKind::Table)
    }
}

pub type AcquisitionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<CatalogSnapshot, AcquisitionError>> + Send + 'a>>;

/// Definitions and effective search path must come from one consistent source
/// snapshot. Blocking providers must offload I/O to avoid blocking the caller
/// runtime; implementations may acquire more definitions than requested.
pub trait CatalogProvider: Send + Sync {
    fn acquire<'a>(&'a self, requests: &'a [TableRequest]) -> AcquisitionFuture<'a>;
}

#[derive(Debug, Clone)]
pub struct InMemoryCatalogProvider {
    result: Result<CatalogSnapshot, AcquisitionError>,
}

impl InMemoryCatalogProvider {
    pub fn new(snapshot: CatalogSnapshot) -> Self {
        Self {
            result: Ok(snapshot),
        }
    }
    pub fn failing(error: AcquisitionError) -> Self {
        Self { result: Err(error) }
    }
}

impl CatalogProvider for InMemoryCatalogProvider {
    fn acquire<'a>(&'a self, _requests: &'a [TableRequest]) -> AcquisitionFuture<'a> {
        Box::pin(std::future::ready(self.result.clone()))
    }
}
