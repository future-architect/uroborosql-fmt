use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions,
};

use super::{error, AcquisitionError, AcquisitionErrorKind, AcquisitionPhase};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TlsMode {
    #[default]
    VerifyFull,
    Disable,
}

#[derive(Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub dbname: String,
    pub password: Option<String>,
    pub tls_mode: TlsMode,
}

impl PostgresConfig {
    pub fn new(
        host: impl Into<String>,
        user: impl Into<String>,
        dbname: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port: 5432,
            user: user.into(),
            dbname: dbname.into(),
            password: None,
            tls_mode: TlsMode::default(),
        }
    }

    pub(super) fn options(&self) -> Result<PgConnectOptions, AcquisitionError> {
        if self.host.is_empty()
            || self.host.starts_with('/')
            || self.host.contains(['\0', ',', '\\'])
            || self.port == 0
            || self.user.is_empty()
            || self.dbname.is_empty()
            || self.user.contains('\0')
            || self.dbname.contains('\0')
        {
            return Err(error(
                AcquisitionPhase::Connect,
                AcquisitionErrorKind::InvalidData,
            ));
        }
        let mut options = PgConnectOptions::new_without_pgpass()
            .host(&self.host)
            .port(self.port)
            .username(&self.user)
            .database(&self.dbname)
            // PGSSLMODE must not weaken the product's default verification.
            .ssl_mode(match self.tls_mode {
                TlsMode::VerifyFull => PgSslMode::VerifyFull,
                TlsMode::Disable => PgSslMode::Disable,
            })
            .disable_statement_logging();
        if let Some(password) = &self.password {
            options = options.password(password);
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_verify_tls_and_reject_non_tcp_hosts() {
        let config = PostgresConfig::new("localhost", "user", "database");
        assert_eq!(config.tls_mode, TlsMode::VerifyFull);
        assert!(matches!(
            config.options().unwrap().get_ssl_mode(),
            PgSslMode::VerifyFull
        ));
        for host in ["", "/tmp", "host,other", "host\0"] {
            assert_eq!(
                PostgresConfig::new(host, "user", "database")
                    .options()
                    .unwrap_err()
                    .kind,
                AcquisitionErrorKind::InvalidData
            );
        }
    }
}
