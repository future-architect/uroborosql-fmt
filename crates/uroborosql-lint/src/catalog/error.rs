use std::{fmt, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionPhase {
    Connect,
    SearchPath,
    Schema,
    Relation,
    Columns,
    Validate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionErrorKind {
    UnsupportedProvider,
    Connection,
    Timeout,
    PermissionDenied,
    InvalidData,
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationField {
    Host,
    Port,
    User,
    Database,
    ConnectTimeout,
    QueryTimeout,
    AcquisitionTimeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutScope {
    Connect,
    Query,
    Acquisition,
}

/// Only classified causes are retained, never driver messages or connection values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionDetail {
    FileProviderUnavailable,
    SnapshotFileUnavailable,
    InvalidSnapshot,
    PostgresProviderUnavailable,
    InvalidConfiguration(ConfigurationField),
    Authentication,
    DatabaseNotFound,
    ConnectionRefused,
    Tls,
    UnsupportedServerVersion,
    Timeout {
        scope: TimeoutScope,
        limit: Duration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionError {
    pub phase: AcquisitionPhase,
    pub kind: AcquisitionErrorKind,
    pub detail: Option<AcquisitionDetail>,
}

impl AcquisitionError {
    pub fn new(phase: AcquisitionPhase, kind: AcquisitionErrorKind) -> Self {
        Self {
            phase,
            kind,
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: AcquisitionDetail) -> Self {
        self.detail = Some(detail);
        self
    }
}

impl std::error::Error for AcquisitionError {}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let action = match self.detail {
            Some(AcquisitionDetail::SnapshotFileUnavailable) => "Catalog snapshot could not be opened. Check that the file exists and is readable.",
            Some(AcquisitionDetail::InvalidSnapshot) => "Catalog snapshot is invalid, incomplete or unsupported. Export a complete snapshot with a supported format version.",
            Some(AcquisitionDetail::FileProviderUnavailable) => "File catalog is unavailable in this build. Use a build with SQLite catalog support.",
            Some(AcquisitionDetail::PostgresProviderUnavailable) => "PostgreSQL catalog is unavailable in this build. Enable the postgres-catalog feature.",
            Some(AcquisitionDetail::InvalidConfiguration(field)) => match field {
                ConfigurationField::Host => "Invalid catalog host. Use a single nonempty DNS name or IP address; Unix sockets and multiple hosts are not supported.",
                ConfigurationField::Port => "Invalid catalog port. Use a port between 1 and 65535.",
                ConfigurationField::User => "Invalid catalog user. Provide a nonempty user without NUL characters.",
                ConfigurationField::Database => "Invalid catalog dbname. Provide a nonempty database name without NUL characters.",
                ConfigurationField::ConnectTimeout => "Invalid catalog connection timeout. Use a positive duration within the system clock's supported range.",
                ConfigurationField::QueryTimeout => "Invalid catalog query timeout. Use a positive duration within the system clock's supported range.",
                ConfigurationField::AcquisitionTimeout => "Invalid catalog acquisition timeout. Use a positive duration within the system clock's supported range.",
            },
            Some(AcquisitionDetail::Authentication) => "Catalog authentication failed. Check the database user/password and server authentication settings.",
            Some(AcquisitionDetail::DatabaseNotFound) => "Catalog database was not found. Check dbname and the selected database server.",
            Some(AcquisitionDetail::ConnectionRefused) => "Catalog connection was refused. Check host, port, whether PostgreSQL is running, and network access.",
            Some(AcquisitionDetail::Tls) => "Catalog TLS connection failed. Check server TLS support, the trusted CA, the connection host name, and the selected TLS mode.",
            Some(AcquisitionDetail::UnsupportedServerVersion) => "Unsupported PostgreSQL version. Use PostgreSQL 14 through 18 for catalog acquisition.",
            Some(AcquisitionDetail::Timeout { scope, limit }) => {
                let scope = match scope {
                    TimeoutScope::Connect => "connection",
                    TimeoutScope::Query => "database operation",
                    TimeoutScope::Acquisition => "overall acquisition",
                };
                return write!(f, "Catalog {scope} timed out after reaching its {limit:?} limit ({:?}). Check network delays, database load or locks, and the timeout limit.", self.phase);
            }
            None => match self.kind {
                AcquisitionErrorKind::UnsupportedProvider => "The configured catalog provider is unavailable in this build.",
                AcquisitionErrorKind::Connection => "Catalog connection failed. Check host, port, database credentials, network access, and TLS settings (server support, trusted CA and host name).",
                AcquisitionErrorKind::Timeout => "Catalog acquisition timed out. Check network delays, database load or locks, and the timeout limit.",
                AcquisitionErrorKind::PermissionDenied if self.phase == AcquisitionPhase::Schema => "Catalog schema access was denied. Check the source role's USAGE privilege on the requested schema. For a snapshot, this is the saved access decision; export a new snapshot with a role that has USAGE.",
                AcquisitionErrorKind::PermissionDenied => "Catalog access was denied. Check the connection role's catalog read privileges.",
                AcquisitionErrorKind::InvalidData => "Catalog data was invalid or incomplete. Check catalog visibility and that the database environment is supported.",
                AcquisitionErrorKind::Read => "Catalog read failed. Check the database connection, server state, and catalog visibility.",
            },
        };
        write!(f, "{action} ({:?}: {:?})", self.phase, self.kind)
    }
}
