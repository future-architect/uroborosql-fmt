use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions,
};

use super::{error, AcquisitionError, AcquisitionErrorKind, AcquisitionPhase};
use crate::catalog::{AcquisitionDetail, ConfigurationField};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TlsMode {
    /// Require TLS and verify the server certificate and host name.
    #[default]
    VerifyFull,
    /// Require TLS and verify the server certificate, without checking the host name.
    VerifyCa,
    /// Require TLS without verifying the server certificate or host name.
    Require,
    /// Connect without TLS.
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
        let invalid_field = if self.host.is_empty()
            || self.host.starts_with('/')
            || self.host.contains(['\0', ',', '\\'])
        {
            Some(ConfigurationField::Host)
        } else if self.port == 0 {
            Some(ConfigurationField::Port)
        } else if self.user.is_empty() || self.user.contains('\0') {
            Some(ConfigurationField::User)
        } else if self.dbname.is_empty() || self.dbname.contains('\0') {
            Some(ConfigurationField::Database)
        } else {
            None
        };
        if let Some(field) = invalid_field {
            return Err(
                error(AcquisitionPhase::Connect, AcquisitionErrorKind::InvalidData)
                    .with_detail(AcquisitionDetail::InvalidConfiguration(field)),
            );
        }
        let mut options = PgConnectOptions::new_without_pgpass()
            .host(&self.host)
            .port(self.port)
            .username(&self.user)
            .database(&self.dbname)
            // PGSSLMODE must not weaken the product's default verification.
            .ssl_mode(match self.tls_mode {
                TlsMode::VerifyFull => PgSslMode::VerifyFull,
                TlsMode::VerifyCa => PgSslMode::VerifyCa,
                TlsMode::Require => PgSslMode::Require,
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

    #[test]
    fn invalid_configuration_identifies_the_field_without_its_value() {
        let mut config = PostgresConfig::new("private-host,other", "user", "database");
        let error = config.options().unwrap_err();
        assert!(error.to_string().contains("single nonempty DNS name or IP"));
        assert!(!format!("{error} {error:?}").contains("private-host"));
        config.host = "localhost".into();
        config.port = 0;
        assert!(config
            .options()
            .unwrap_err()
            .to_string()
            .contains("port between 1 and 65535"));
        config.port = 5432;
        config.user = "private-user\0".into();
        let error = config.options().unwrap_err();
        assert!(error.to_string().contains("nonempty user"));
        assert!(!format!("{error} {error:?}").contains("private-user"));
        config.user = "user".into();
        config.dbname.clear();
        assert!(config
            .options()
            .unwrap_err()
            .to_string()
            .contains("nonempty database name"));
    }
}
