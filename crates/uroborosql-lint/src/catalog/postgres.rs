use std::{future::Future, time::Duration};

use sqlx::{Connection, PgConnection};

use super::{
    AcquisitionDetail, AcquisitionError, AcquisitionErrorKind, AcquisitionFuture, AcquisitionPhase,
    CatalogProvider, CatalogSnapshot, TableRequest, TimeoutScope,
};

mod config;
mod metadata;

pub use config::{PostgresConfig, TlsMode};
use metadata::read_snapshot;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const ACQUISITION_TIMEOUT: Duration = Duration::from_secs(10);

pub struct PostgresCatalogProvider {
    config: PostgresConfig,
}

impl PostgresCatalogProvider {
    pub fn new(config: PostgresConfig) -> Self {
        Self { config }
    }

    async fn acquire_snapshot(
        &self,
        requests: &[TableRequest],
        phase: &mut AcquisitionPhase,
    ) -> Result<CatalogSnapshot, AcquisitionError> {
        let options = self.config.options()?;
        let mut connection =
            tokio::time::timeout(CONNECT_TIMEOUT, PgConnection::connect_with(&options))
                .await
                .map_err(|_| timeout_error(*phase, TimeoutScope::Connect, CONNECT_TIMEOUT))?
                .map_err(|err| read_error(*phase, err))?;
        let result = read_snapshot(&mut connection, requests, phase).await;
        // A cancelled future owns and drops the socket; no pool can return it to another analysis.
        if result.is_ok() {
            query(phase, AcquisitionPhase::Validate, connection.close()).await?;
        }
        result
    }
}

impl CatalogProvider for PostgresCatalogProvider {
    fn acquire<'a>(&'a self, requests: &'a [TableRequest]) -> AcquisitionFuture<'a> {
        Box::pin(async move {
            let mut phase = AcquisitionPhase::Connect;
            tokio::time::timeout(
                ACQUISITION_TIMEOUT,
                self.acquire_snapshot(requests, &mut phase),
            )
            .await
            .map_err(|_| timeout_error(phase, TimeoutScope::Acquisition, ACQUISITION_TIMEOUT))?
        })
    }
}

async fn query<T>(
    phase: &mut AcquisitionPhase,
    next: AcquisitionPhase,
    future: impl Future<Output = Result<T, sqlx::Error>>,
) -> Result<T, AcquisitionError> {
    *phase = next;
    tokio::time::timeout(QUERY_TIMEOUT, future)
        .await
        .map_err(|_| timeout_error(next, TimeoutScope::Query, QUERY_TIMEOUT))?
        .map_err(|err| read_error(next, err))
}

fn error(phase: AcquisitionPhase, kind: AcquisitionErrorKind) -> AcquisitionError {
    AcquisitionError::new(phase, kind)
}

fn timeout_error(
    phase: AcquisitionPhase,
    scope: TimeoutScope,
    limit: Duration,
) -> AcquisitionError {
    error(phase, AcquisitionErrorKind::Timeout)
        .with_detail(AcquisitionDetail::Timeout { scope, limit })
}

fn read_error(phase: AcquisitionPhase, err: sqlx::Error) -> AcquisitionError {
    let kind = match &err {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("42501") => {
            AcquisitionErrorKind::PermissionDenied
        }
        sqlx::Error::ColumnDecode { .. }
        | sqlx::Error::Decode(_)
        | sqlx::Error::ColumnNotFound(_) => AcquisitionErrorKind::InvalidData,
        _ if phase == AcquisitionPhase::Connect => AcquisitionErrorKind::Connection,
        _ => AcquisitionErrorKind::Read,
    };
    let detail = match &err {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("28P01" | "28000") => Some(AcquisitionDetail::Authentication),
            Some("3D000") => Some(AcquisitionDetail::DatabaseNotFound),
            _ => None,
        },
        sqlx::Error::Tls(_) => Some(AcquisitionDetail::Tls),
        sqlx::Error::Io(err) if err.kind() == std::io::ErrorKind::ConnectionRefused => {
            Some(AcquisitionDetail::ConnectionRefused)
        }
        _ => None,
    };
    AcquisitionError {
        phase,
        kind,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn driver_failures_give_actions_without_retaining_raw_messages() {
        for (raw, expected) in [
            (
                sqlx::Error::Tls("private-connection-secret".into()),
                "trusted CA",
            ),
            (
                sqlx::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "private-connection-secret",
                )),
                "host, port",
            ),
            (
                sqlx::Error::Protocol("private-connection-secret".into()),
                "network access",
            ),
        ] {
            let error = read_error(AcquisitionPhase::Connect, raw);
            assert!(error.to_string().contains(expected));
            assert!(!format!("{error} {error:?}").contains("private-connection-secret"));
            assert!(error.source().is_none());
        }
    }

    #[test]
    fn io_handshake_failures_include_tls_guidance_without_assuming_a_tls_cause() {
        // SQLx's rustls handshake propagates certificate failures through
        // io::Error(InvalidData), not Error::Tls. Other I/O failures can use
        // the same kind, so preserve the unknown cause rather than guessing.
        let raw = sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "private-certificate-details",
        ));
        let error = read_error(AcquisitionPhase::Connect, raw);
        assert_eq!(error.detail, None);
        assert!(error.to_string().contains("trusted CA and host name"));
        assert!(!format!("{error} {error:?}").contains("private-certificate-details"));
        assert!(error.source().is_none());
    }
}
