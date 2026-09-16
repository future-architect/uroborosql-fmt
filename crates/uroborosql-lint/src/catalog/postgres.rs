use std::{future::Future, time::Duration};

use sqlx::{Connection, PgConnection};

use super::{
    AcquisitionError, AcquisitionErrorKind, AcquisitionFuture, AcquisitionPhase, CatalogProvider,
    CatalogSnapshot, TableRequest,
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
                .map_err(|_| error(*phase, AcquisitionErrorKind::Timeout))?
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
            .map_err(|_| error(phase, AcquisitionErrorKind::Timeout))?
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
        .map_err(|_| error(next, AcquisitionErrorKind::Timeout))?
        .map_err(|err| read_error(next, err))
}

fn error(phase: AcquisitionPhase, kind: AcquisitionErrorKind) -> AcquisitionError {
    AcquisitionError { phase, kind }
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
    error(phase, kind)
}
