use super::{config_store::ResolvedDbConfig, ResolvedLintConfig};
#[cfg(any(not(feature = "postgres-catalog"), not(feature = "sqlite-catalog")))]
use crate::catalog::{
    AcquisitionDetail, AcquisitionError, AcquisitionErrorKind, AcquisitionPhase,
    InMemoryCatalogProvider,
};

use crate::catalog::CatalogProvider;

impl ResolvedLintConfig {
    /// Selects a catalog source without opening connections or files.
    ///
    /// Unavailable optional providers fail only when acquisition is requested.
    /// Pass the returned provider to `Linter::run_async`; callers may instead
    /// supply their own provider explicitly.
    pub fn catalog_provider(&self) -> Option<Box<dyn CatalogProvider>> {
        let db = self.db.as_ref()?;
        Some(match db {
            ResolvedDbConfig::Server { .. } => server_provider(db),
            ResolvedDbConfig::File { path } => file_provider(path),
        })
    }
}

#[cfg(any(not(feature = "postgres-catalog"), not(feature = "sqlite-catalog")))]
fn unavailable(detail: AcquisitionDetail) -> Box<dyn CatalogProvider> {
    Box::new(InMemoryCatalogProvider::failing(
        AcquisitionError::new(
            AcquisitionPhase::Connect,
            AcquisitionErrorKind::UnsupportedProvider,
        )
        .with_detail(detail),
    ))
}

#[cfg(not(feature = "postgres-catalog"))]
fn server_provider(_: &ResolvedDbConfig) -> Box<dyn CatalogProvider> {
    unavailable(AcquisitionDetail::PostgresProviderUnavailable)
}

#[cfg(feature = "postgres-catalog")]
fn server_provider(db: &ResolvedDbConfig) -> Box<dyn CatalogProvider> {
    use super::lint_config::ConfigTlsMode;
    use crate::catalog::postgres::{PostgresCatalogProvider, PostgresConfig, TlsMode};
    use std::time::Duration;
    let ResolvedDbConfig::Server {
        host,
        port,
        user,
        password,
        dbname,
        tls_mode,
        timeouts,
    } = db
    else {
        unreachable!("server provider requires server config");
    };
    let mut config = PostgresConfig::new(host.clone(), user.clone(), dbname.clone());
    config.port = port.unwrap_or(5432);
    config.password = password.clone();
    config.tls_mode = match tls_mode {
        ConfigTlsMode::VerifyFull => TlsMode::VerifyFull,
        ConfigTlsMode::VerifyCa => TlsMode::VerifyCa,
        ConfigTlsMode::Require => TlsMode::Require,
        ConfigTlsMode::Disable => TlsMode::Disable,
    };
    if let Some(ms) = timeouts.connect_ms {
        config.timeouts.connect = Duration::from_millis(ms);
    }
    if let Some(ms) = timeouts.query_ms {
        config.timeouts.query = Duration::from_millis(ms);
    }
    if let Some(ms) = timeouts.acquisition_ms {
        config.timeouts.acquisition = Duration::from_millis(ms);
    }
    Box::new(PostgresCatalogProvider::new(config))
}

#[cfg(feature = "sqlite-catalog")]
fn file_provider(path: &std::path::Path) -> Box<dyn CatalogProvider> {
    Box::new(crate::catalog::sqlite::SqliteCatalogProvider::new(path))
}

#[cfg(not(feature = "sqlite-catalog"))]
fn file_provider(_: &std::path::Path) -> Box<dyn CatalogProvider> {
    unavailable(AcquisitionDetail::FileProviderUnavailable)
}
