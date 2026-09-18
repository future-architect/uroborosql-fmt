use crate::args::ExportArgs;

#[cfg(all(feature = "postgres-catalog", feature = "sqlite-catalog"))]
fn config(args: &ExportArgs) -> uroborosql_lint::catalog::postgres::PostgresConfig {
    use crate::args::ExportTlsMode;
    use std::time::Duration;
    use uroborosql_lint::catalog::{
        postgres::{PostgresConfig, TlsMode},
        sqlite::export::default_timeouts,
    };
    let mut config = PostgresConfig::new(&args.host, &args.user, &args.dbname);
    config.port = args.port;
    config.tls_mode = match args.tls_mode {
        ExportTlsMode::VerifyFull => TlsMode::VerifyFull,
        ExportTlsMode::VerifyCa => TlsMode::VerifyCa,
        ExportTlsMode::Require => TlsMode::Require,
        ExportTlsMode::Disable => TlsMode::Disable,
    };
    config.timeouts = default_timeouts();
    if let Some(ms) = args.connect_timeout_ms {
        config.timeouts.connect = Duration::from_millis(ms);
    }
    if let Some(ms) = args.query_timeout_ms {
        config.timeouts.query = Duration::from_millis(ms);
    }
    if let Some(ms) = args.acquisition_timeout_ms {
        config.timeouts.acquisition = Duration::from_millis(ms);
    }
    config
}

#[cfg(all(feature = "postgres-catalog", feature = "sqlite-catalog"))]
pub async fn run(args: ExportArgs) -> u8 {
    use uroborosql_lint::catalog::sqlite::export::{default_output_path, export_catalog};
    let output = match args
        .output
        .clone()
        .map(Ok)
        .unwrap_or_else(default_output_path)
    {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Catalog export failed: {error}");
            return 2;
        }
    };
    match export_catalog(&config(&args), &output).await {
        Ok(()) => {
            eprintln!("Catalog exported to {}", output.display());
            0
        }
        Err(error) => {
            eprintln!("Catalog export failed for {}: {error}", output.display());
            2
        }
    }
}

#[cfg(not(all(feature = "postgres-catalog", feature = "sqlite-catalog")))]
pub async fn run(_: ExportArgs) -> u8 {
    eprintln!("Catalog export is unavailable in this build. Enable PostgreSQL and SQLite catalog support.");
    2
}

#[cfg(all(test, feature = "postgres-catalog", feature = "sqlite-catalog"))]
mod tests {
    use super::*;
    use crate::args::{Cli, Command};
    use clap::Parser;
    use std::time::Duration;
    fn parsed(extra: &[&str]) -> ExportArgs {
        let args = Cli::try_parse_from(
            [
                "lint",
                "export-catalog",
                "--host",
                "host",
                "--user",
                "user",
                "--dbname",
                "db",
            ]
            .into_iter()
            .chain(extra.iter().copied()),
        )
        .unwrap();
        let Some(Command::ExportCatalog(args)) = args.command else {
            panic!("missing export");
        };
        args
    }
    #[test]
    fn defaults_partial_and_explicit_timeouts_are_independent_of_lint_config() {
        for (extra, expected) in [
            (vec![], [5000, 30000, 120000]),
            (vec!["--connect-timeout-ms", "12"], [12, 30000, 120000]),
            (
                vec![
                    "--connect-timeout-ms",
                    "12",
                    "--query-timeout-ms",
                    "34",
                    "--acquisition-timeout-ms",
                    "56",
                ],
                [12, 34, 56],
            ),
        ] {
            let c = config(&parsed(&extra));
            assert_eq!(
                [c.timeouts.connect, c.timeouts.query, c.timeouts.acquisition],
                expected.map(Duration::from_millis)
            );
        }
    }
}
