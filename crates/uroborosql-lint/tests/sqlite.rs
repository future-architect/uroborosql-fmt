#![cfg(feature = "sqlite-catalog")]
use sqlx::{sqlite::SqliteConnectOptions, Connection, SqliteConnection};
use uroborosql_lint::catalog::{
    sqlite::SqliteCatalogProvider, AbsenceKind, AcquisitionErrorKind, CatalogProvider, Lookup,
    TableRequest, UnknownReason,
};

async fn fixture(path: &std::path::Path) -> SqliteConnection {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../src/catalog/sqlite/schema.sql"))
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO pg_namespace VALUES(1,'app'),(2,'public'),(3,'hidden'),(4,'Public');
    INSERT INTO snapshot_schema_access VALUES(1,1),(2,1),(3,0),(4,1);
    INSERT INTO snapshot_search_path VALUES(1,1),(2,2);
    INSERT INTO pg_class VALUES(10,1,'users','r',3),(11,2,'users','r',1),(12,1,'shadow','v',1),(13,2,'shadow','r',1),(14,4,'Users','p',0);
    INSERT INTO pg_attribute VALUES(10,1,'id',0),(10,2,'dropped',1),(10,3,'Name',0),(10,-1,'ctid',0),(11,1,'public_id',0),(12,1,'view_col',0),(13,1,'table_col',0);
    INSERT INTO snapshot_meta VALUES(1,1,180000,'db','login','role','2026-09-18T01:02:03.000001Z','database_catalog',1,4,5,7,2);").execute(&mut connection).await.unwrap();
    connection
}
fn request(schema: Option<&str>, name: &str) -> TableRequest {
    TableRequest {
        schema: schema.map(str::to_owned),
        name: name.into(),
    }
}

#[tokio::test]
async fn resolves_saved_path_spelling_privileges_and_column_order_read_only() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("catalog.sqlite");
    fixture(&path).await.close().await.unwrap();
    let before = std::fs::read(&path).unwrap();
    let requests = [
        request(None, "users"),
        request(Some("public"), "users"),
        request(None, "shadow"),
        request(Some("hidden"), "users"),
        request(Some("missing"), "users"),
        request(Some("Public"), "Users"),
        request(Some("pg_temp"), "users"),
        request(None, "missing"),
    ];
    let snapshot = SqliteCatalogProvider::new(&path)
        .acquire(&requests)
        .await
        .unwrap();
    let Lookup::Found(table) = snapshot.lookup(&requests[0]) else {
        panic!("not found");
    };
    assert_eq!(table.schema, "app");
    assert_eq!(
        table
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "Name"]
    );
    assert_eq!(table.system_columns[0].name, "ctid");
    assert!(matches!(
        table.column("name"),
        Lookup::Absent(AbsenceKind::Column)
    ));
    assert!(matches!(
        table.column("dropped"),
        Lookup::Absent(AbsenceKind::Column)
    ));
    assert!(
        matches!(snapshot.lookup(&requests[1]),Lookup::Found(t) if t.columns[0].name == "public_id")
    );
    assert_eq!(
        snapshot.lookup(&requests[2]),
        Lookup::Unknown(UnknownReason::UnsupportedRelation)
    );
    assert!(
        matches!(snapshot.lookup(&requests[3]),Lookup::Unavailable(e) if e.kind == AcquisitionErrorKind::PermissionDenied)
    );
    assert_eq!(
        snapshot.lookup(&requests[4]),
        Lookup::Absent(AbsenceKind::Schema)
    );
    assert!(matches!(snapshot.lookup(&requests[5]),Lookup::Found(t) if t.columns.is_empty()));
    assert_eq!(
        snapshot.lookup(&requests[6]),
        Lookup::Unknown(UnknownReason::UnsupportedSyntax)
    );
    assert_eq!(
        snapshot.lookup(&requests[7]),
        Lookup::Absent(AbsenceKind::Table)
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[tokio::test]
async fn rejects_incomplete_and_malformed_values_without_trusting_declared_constraints() {
    for mutation in [
        "UPDATE snapshot_meta SET format_version=2", "UPDATE snapshot_meta SET complete=0", "UPDATE snapshot_meta SET scope='partial'", "UPDATE snapshot_meta SET server_version_num=190000", "UPDATE snapshot_meta SET captured_at='2026-02-30T00:00:00Z'", "UPDATE snapshot_meta SET attribute_count=8", "DELETE FROM snapshot_meta", "DROP TABLE pg_attribute", "ALTER TABLE pg_class RENAME COLUMN relnatts TO missing", "UPDATE snapshot_schema_access SET usage_allowed=2 WHERE nspoid=1", "DELETE FROM snapshot_schema_access WHERE nspoid=3", "UPDATE snapshot_search_path SET position=4 WHERE position=2", "UPDATE snapshot_schema_access SET usage_allowed=0 WHERE nspoid=1", "DELETE FROM pg_attribute WHERE attrelid=10 AND attnum=2; UPDATE snapshot_meta SET attribute_count=6", "UPDATE pg_attribute SET attnum=0 WHERE attrelid=10 AND attnum=-1", "UPDATE pg_attribute SET attisdropped=1 WHERE attnum=-1", "UPDATE pg_class SET relkind='table' WHERE oid=10", "UPDATE pg_namespace SET oid=4294967296 WHERE oid=4", "UPDATE pg_class SET relnamespace=999 WHERE oid=10", "UPDATE pg_attribute SET attrelid=999 WHERE attrelid=11", "UPDATE pg_attribute SET attname='id' WHERE attrelid=10 AND attnum=3", "UPDATE pg_class SET relnatts=1.5 WHERE oid=10", "CREATE TABLE replacement AS SELECT * FROM pg_namespace; INSERT INTO replacement SELECT * FROM pg_namespace WHERE oid=1; DROP TABLE pg_namespace; ALTER TABLE replacement RENAME TO pg_namespace; UPDATE snapshot_meta SET namespace_count=5", "CREATE TABLE replacement AS SELECT * FROM snapshot_meta; INSERT INTO replacement SELECT * FROM snapshot_meta; DROP TABLE snapshot_meta; ALTER TABLE replacement RENAME TO snapshot_meta",
    ] {
        let temp = tempfile::tempdir().unwrap(); let path = temp.path().join("catalog.sqlite");
        let mut conn = fixture(&path).await;
        sqlx::raw_sql("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;").execute(&mut conn).await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(mutation)).execute(&mut conn).await.unwrap();
        conn.close().await.unwrap();
        assert!(SqliteCatalogProvider::new(&path).acquire(&[request(None,"missing")]).await.is_err(),"accepted {mutation}");
    }
}

#[tokio::test]
async fn missing_corrupt_and_empty_files_fail_without_creating_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("missing.sqlite");
    let provider = SqliteCatalogProvider::new(&path);
    assert!(!path.exists());
    assert!(provider.acquire(&[request(None, "users")]).await.is_err());
    assert!(!path.exists());
    for bytes in [b"".as_slice(), b"not a sqlite database"] {
        std::fs::write(&path, bytes).unwrap();
        assert!(provider.acquire(&[request(None, "users")]).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
