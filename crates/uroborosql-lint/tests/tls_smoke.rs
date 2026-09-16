#![cfg(feature = "postgres-catalog")]

use uroborosql_lint::catalog::{
    postgres::{PostgresCatalogProvider, PostgresConfig},
    AcquisitionErrorKind, AcquisitionPhase, CatalogProvider,
};

/// The runner checks the server side of the handshake. The peer deliberately
/// closes after StartupMessage, so even verified TLS ends in a connection error.
#[tokio::test]
#[ignore = "requires TLS peer; use tests/postgres/tls_smoke.py"]
async fn tls_peer_connection() {
    let mut config = PostgresConfig::new(
        std::env::var("CATALOG_TEST_HOST").unwrap(),
        "tls_smoke",
        "tls_smoke",
    );
    config.port = std::env::var("CATALOG_TEST_PORT").unwrap().parse().unwrap();
    config.password = Some("public-test-only".into());
    let error = PostgresCatalogProvider::new(config)
        .acquire(&[])
        .await
        .unwrap_err();
    assert_eq!(error.phase, AcquisitionPhase::Connect);
    assert_eq!(error.kind, AcquisitionErrorKind::Connection);
}
