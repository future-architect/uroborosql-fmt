//! Export one consistent PostgreSQL catalog and atomically publish a portable file.
use super::{
    data::{self, Attribute, Meta, Relation, SnapshotData},
    invalid, read_error,
};
use crate::catalog::{
    postgres::{self, CatalogTimeouts, PostgresConfig},
    AcquisitionError, AcquisitionPhase, TimeoutScope,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
    ConnectOptions, Connection, PgConnection, Row, SqliteConnection,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Export limits are longer than request-scoped lint acquisition defaults.
pub fn default_timeouts() -> CatalogTimeouts {
    CatalogTimeouts {
        connect: Duration::from_secs(5),
        query: Duration::from_secs(30),
        acquisition: Duration::from_secs(120),
    }
}

/// Generate the UTC default name. Existing names use the same replacement policy.
pub fn default_output_path() -> Result<PathBuf, ExportError> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ExportError::File("system clock"))?
        .as_secs();
    Ok(PathBuf::from(timestamp_filename(seconds)))
}

fn timestamp_filename(seconds: u64) -> String {
    // Gregorian civil date from Unix days, using 400-year eras.
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = month_index + if month_index < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let time = seconds % 86400;
    format!(
        "catalog-{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z.sqlite",
        time / 3600,
        time / 60 % 60,
        time % 60
    )
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{0}")]
    Acquisition(#[from] AcquisitionError),
    #[error("Catalog export filesystem operation failed ({0}). Check the output path and filesystem permissions.")]
    File(&'static str),
}

/// The supplied output path is relative to the caller's working directory.
/// Existing regular files are atomically replaced only after validation and close.
pub async fn export_catalog(config: &PostgresConfig, output: &Path) -> Result<(), ExportError> {
    config.timeouts.validate()?;
    let deadline = tokio::time::Instant::now() + config.timeouts.acquisition;
    let data = tokio::time::timeout_at(deadline, acquire(config))
        .await
        .map_err(|_| overall_timeout(config.timeouts))??;
    publish_snapshot(&data, output, deadline, config.timeouts).await
}

async fn publish_snapshot(
    data: &SnapshotData,
    output: &Path,
    deadline: tokio::time::Instant,
    timeouts: CatalogTimeouts,
) -> Result<(), ExportError> {
    check_destination(output)?;
    let temporary = Temporary::new(output)?;
    let options = SqliteConnectOptions::new()
        .filename(&temporary.path)
        .create_if_missing(false)
        .journal_mode(SqliteJournalMode::Delete)
        .foreign_keys(true)
        .disable_statement_logging();
    // Recover ownership even when file opening crosses the deadline, then close before cleanup.
    // Local OS file operations and worker shutdown are not a strict wall-clock guarantee.
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(read_error)?;
    if tokio::time::Instant::now() >= deadline {
        connection.close().await.map_err(read_error)?;
        return Err(overall_timeout(timeouts));
    }
    let result = tokio::time::timeout_at(deadline, write_and_validate(&mut connection, data))
        .await
        .map_err(|_| overall_timeout(timeouts))
        .and_then(|r| r.map_err(ExportError::from));
    // Always close the worker before removing its temporary file, including timeouts.
    let closed = connection.close().await.map_err(read_error);
    result?;
    closed?;
    // Own the read connection outside the deadline so cancellation still awaits close.
    let reader = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&temporary.path)
            .read_only(true)
            .disable_statement_logging(),
    )
    .await
    .map_err(read_error)?;
    validate_and_close(reader, deadline, timeouts).await?;
    if tokio::time::Instant::now() >= deadline {
        return Err(overall_timeout(timeouts));
    }
    check_destination(output)?;
    // No timeout may race publication: once rename starts its result is authoritative.
    publish_file(&temporary.path, output)
}

async fn validate_and_close(
    mut connection: SqliteConnection,
    deadline: tokio::time::Instant,
    timeouts: CatalogTimeouts,
) -> Result<(), ExportError> {
    let result = tokio::time::timeout_at(deadline, async {
        let mut transaction = connection.begin().await.map_err(read_error)?;
        data::read_validated(&mut transaction).await?;
        transaction.commit().await.map_err(read_error)
    })
    .await
    .map_err(|_| overall_timeout(timeouts))
    .and_then(|r| r.map_err(ExportError::from));
    // SQLx close waits for its worker; dropping a timed-out read future alone does not.
    let closed = connection.close().await.map_err(read_error);
    result?;
    closed?;
    Ok(())
}

fn publish_file(source: &Path, output: &Path) -> Result<(), ExportError> {
    fs::rename(source, output).map_err(|_| ExportError::File("publish"))
}

fn overall_timeout(timeouts: CatalogTimeouts) -> ExportError {
    postgres::timeout_error(
        AcquisitionPhase::Validate,
        TimeoutScope::Acquisition,
        timeouts.acquisition,
    )
    .into()
}

async fn acquire(config: &PostgresConfig) -> Result<SnapshotData, AcquisitionError> {
    let options = config.options()?;
    let mut connection = tokio::time::timeout(
        config.timeouts.connect,
        PgConnection::connect_with(&options),
    )
    .await
    .map_err(|_| {
        postgres::timeout_error(
            AcquisitionPhase::Connect,
            TimeoutScope::Connect,
            config.timeouts.connect,
        )
    })?
    .map_err(|e| postgres::read_error(AcquisitionPhase::Connect, e))?;
    let result = acquire_rows(&mut connection, config.timeouts.query).await;
    if result.is_ok() {
        postgres::query(
            &mut AcquisitionPhase::Validate,
            AcquisitionPhase::Validate,
            config.timeouts.query,
            connection.close(),
        )
        .await?;
    }
    result
}

async fn acquire_rows(
    connection: &mut PgConnection,
    limit: Duration,
) -> Result<SnapshotData, AcquisitionError> {
    let mut phase = AcquisitionPhase::SearchPath;
    macro_rules! query {
        ($q:expr) => {
            postgres::query(&mut phase, AcquisitionPhase::Validate, limit, $q).await?
        };
    }
    query!(
        sqlx::query("BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *connection)
    );
    let row = query!(sqlx::query("SELECT pg_catalog.current_setting('server_version_num')::bigint AS version, pg_catalog.current_database()::text AS database, session_user::text AS session_user, current_user::text AS current_user, pg_catalog.to_char(pg_catalog.transaction_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS captured_at").fetch_one(&mut *connection));
    let mut data = SnapshotData {
        meta: Meta {
            version: row.try_get("version").map_err(read_error)?,
            database: row.try_get("database").map_err(read_error)?,
            session_user: row.try_get("session_user").map_err(read_error)?,
            current_user: row.try_get("current_user").map_err(read_error)?,
            captured_at: row.try_get("captured_at").map_err(read_error)?,
            counts: [0; 4],
        },
        ..SnapshotData::default()
    };
    if !(14..=18).contains(&(data.meta.version / 10000)) {
        return Err(
            invalid().with_detail(crate::catalog::AcquisitionDetail::UnsupportedServerVersion)
        );
    }
    for row in query!(sqlx::query("SELECT oid::bigint AS oid, nspname::text AS name, pg_catalog.has_schema_privilege(oid, 'USAGE') AS usable FROM pg_catalog.pg_namespace").fetch_all(&mut *connection)) {
        let oid = pg_oid(&row,"oid")?;
        data.namespaces.insert(oid,row.try_get("name").map_err(read_error)?);
        data.access.insert(oid,row.try_get("usable").map_err(read_error)?);
    }
    for row in query!(sqlx::query("SELECT oid::bigint AS oid, relnamespace::bigint AS namespace, relname::text AS name, relkind::text AS kind, relnatts::bigint AS count FROM pg_catalog.pg_class").fetch_all(&mut *connection)) {
        data.relations.insert(pg_oid(&row,"oid")?, Relation { namespace: pg_oid(&row,"namespace")?, name: row.try_get("name").map_err(read_error)?, kind: row.try_get("kind").map_err(read_error)?, count: row.try_get("count").map_err(read_error)? });
    }
    for row in query!(sqlx::query("SELECT attrelid::bigint AS relation, attnum, attname::text AS name, attisdropped FROM pg_catalog.pg_attribute").fetch_all(&mut *connection)) {
        data.attributes.insert((pg_oid(&row,"relation")?,row.try_get("attnum").map_err(read_error)?), Attribute { name: row.try_get("name").map_err(read_error)?, dropped: row.try_get("attisdropped").map_err(read_error)? });
    }
    for row in query!(sqlx::query("SELECT p.position::bigint AS position, n.oid::bigint AS oid FROM pg_catalog.unnest(pg_catalog.current_schemas(true)) WITH ORDINALITY p(name, position) JOIN pg_catalog.pg_namespace n ON n.nspname = p.name ORDER BY p.position").fetch_all(&mut *connection)) {
        data.path.insert(row.try_get("position").map_err(read_error)?,pg_oid(&row,"oid")?);
    }
    data.meta.counts = [
        data.namespaces.len() as i64,
        data.relations.len() as i64,
        data.attributes.len() as i64,
        data.path.len() as i64,
    ];
    data.validate()?;
    query!(sqlx::query("COMMIT").execute(&mut *connection));
    Ok(data)
}
fn pg_oid(row: &sqlx::postgres::PgRow, name: &str) -> Result<u32, AcquisitionError> {
    let oid: i64 = row.try_get(name).map_err(read_error)?;
    oid.try_into().map_err(|_| invalid())
}

async fn write_and_validate(
    connection: &mut SqliteConnection,
    data: &SnapshotData,
) -> Result<(), AcquisitionError> {
    let mut tx = connection.begin().await.map_err(read_error)?;
    sqlx::raw_sql(include_str!("schema.sql"))
        .execute(&mut *tx)
        .await
        .map_err(read_error)?;
    for (oid, name) in &data.namespaces {
        sqlx::query("INSERT INTO pg_namespace VALUES (?,?)")
            .bind(i64::from(*oid))
            .bind(name)
            .execute(&mut *tx)
            .await
            .map_err(read_error)?;
    }
    for (oid, r) in &data.relations {
        sqlx::query("INSERT INTO pg_class VALUES (?,?,?,?,?)")
            .bind(i64::from(*oid))
            .bind(i64::from(r.namespace))
            .bind(&r.name)
            .bind(&r.kind)
            .bind(r.count)
            .execute(&mut *tx)
            .await
            .map_err(read_error)?;
    }
    for ((oid, number), a) in &data.attributes {
        sqlx::query("INSERT INTO pg_attribute VALUES (?,?,?,?)")
            .bind(i64::from(*oid))
            .bind(*number)
            .bind(&a.name)
            .bind(a.dropped)
            .execute(&mut *tx)
            .await
            .map_err(read_error)?;
    }
    for (oid, access) in &data.access {
        sqlx::query("INSERT INTO snapshot_schema_access VALUES (?,?)")
            .bind(i64::from(*oid))
            .bind(*access)
            .execute(&mut *tx)
            .await
            .map_err(read_error)?;
    }
    for (position, oid) in &data.path {
        sqlx::query("INSERT INTO snapshot_search_path VALUES (?,?)")
            .bind(position)
            .bind(i64::from(*oid))
            .execute(&mut *tx)
            .await
            .map_err(read_error)?;
    }
    let m = &data.meta;
    sqlx::query("INSERT INTO snapshot_meta VALUES (1,1,?,?,?,?,?,'database_catalog',1,?,?,?,?)")
        .bind(m.version)
        .bind(&m.database)
        .bind(&m.session_user)
        .bind(&m.current_user)
        .bind(&m.captured_at)
        .bind(m.counts[0])
        .bind(m.counts[1])
        .bind(m.counts[2])
        .bind(m.counts[3])
        .execute(&mut *tx)
        .await
        .map_err(read_error)?;
    data::read_validated(&mut tx).await?;
    tx.commit().await.map_err(read_error)?;
    Ok(())
}

fn check_destination(output: &Path) -> Result<(), ExportError> {
    match fs::symlink_metadata(output) {
        Ok(m) if !m.file_type().is_file() => {
            Err(ExportError::File("destination must be a regular file"))
        }
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(ExportError::File("inspect destination")),
    }
}
struct Temporary {
    path: PathBuf,
}
impl Temporary {
    fn new(output: &Path) -> Result<Self, ExportError> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        for _ in 0..100 {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| ExportError::File("system clock"))?
                .as_nanos();
            let path = parent.join(format!(
                ".catalog-{}-{stamp}-{}.tmp",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(ExportError::File("create temporary file")),
            }
        }
        Err(ExportError::File("create temporary file"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CatalogProvider, Lookup, TableRequest};
    #[test]
    fn default_name_is_utc_without_collision_suffixes() {
        assert_eq!(timestamp_filename(0), "catalog-19700101T000000Z.sqlite");
        assert_eq!(
            timestamp_filename(1709251199),
            "catalog-20240229T235959Z.sqlite"
        );
        assert_eq!(
            timestamp_filename(1709251200),
            "catalog-20240301T000000Z.sqlite"
        );
    }
    fn data(name: &str) -> SnapshotData {
        SnapshotData {
            meta: Meta {
                version: 180000,
                database: "db".into(),
                session_user: "login".into(),
                current_user: "role".into(),
                captured_at: "2026-09-18T00:00:00Z".into(),
                counts: [1, 1, 1, 1],
            },
            namespaces: [(1, "public".into())].into(),
            access: [(1, true)].into(),
            path: [(1, 1)].into(),
            relations: [(
                10,
                Relation {
                    namespace: 1,
                    name: "users".into(),
                    kind: "r".into(),
                    count: 1,
                },
            )]
            .into(),
            attributes: [(
                (10, 1),
                Attribute {
                    name: name.into(),
                    dropped: false,
                },
            )]
            .into(),
        }
    }
    async fn publish(data: &SnapshotData, output: &Path) -> Result<(), ExportError> {
        publish_snapshot(
            data,
            output,
            tokio::time::Instant::now() + Duration::from_secs(10),
            default_timeouts(),
        )
        .await
    }
    #[test]
    fn failed_atomic_replacement_preserves_previous_destination() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("catalog.sqlite");
        fs::write(&output, b"previous").unwrap();
        assert!(publish_file(&temp.path().join("missing"), &output).is_err());
        assert_eq!(fs::read(output).unwrap(), b"previous");
    }

    #[cfg(windows)]
    #[test]
    fn windows_sharing_violation_preserves_previous_destination() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("catalog.sqlite");
        let source = temp.path().join("temporary");
        fs::write(&output, b"previous").unwrap();
        fs::write(&source, b"replacement").unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&output)
            .unwrap();
        assert!(publish_file(&source, &output).is_err());
        drop(lock);
        assert_eq!(fs::read(output).unwrap(), b"previous");
    }

    #[tokio::test]
    async fn write_deadline_does_not_publish_later_and_cleans_temporary_files() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("catalog.sqlite");
        fs::write(&output, b"previous").unwrap();
        let mut large = data("new");
        for oid in 11..10011 {
            large.relations.insert(
                oid,
                Relation {
                    namespace: 1,
                    name: format!("table_{oid}"),
                    kind: "r".into(),
                    count: 0,
                },
            );
        }
        large.meta.counts[1] = large.relations.len() as i64;
        let mut timeouts = default_timeouts();
        timeouts.acquisition = Duration::from_millis(10);
        let result = publish_snapshot(
            &large,
            &output,
            tokio::time::Instant::now() + timeouts.acquisition,
            timeouts,
        )
        .await;
        assert!(
            matches!(result, Err(ExportError::Acquisition(e)) if e.kind == crate::catalog::AcquisitionErrorKind::Timeout)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(fs::read(output).unwrap(), b"previous");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn readonly_validation_timeout_and_failure_close_before_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("temporary.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let mut writer = SqliteConnection::connect_with(&options).await.unwrap();
        write_and_validate(&mut writer, &data("new")).await.unwrap();
        let reader = SqliteConnection::connect_with(&options.clone().read_only(true))
            .await
            .unwrap();
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut writer)
            .await
            .unwrap();
        let unlock_started = std::sync::atomic::AtomicBool::new(false);
        let unlock = async {
            tokio::time::sleep(Duration::from_millis(40)).await;
            unlock_started.store(true, Ordering::SeqCst);
            sqlx::query("ROLLBACK").execute(&mut writer).await.unwrap();
        };
        let validation = async {
            let result = validate_and_close(
                reader,
                tokio::time::Instant::now() + Duration::from_millis(10),
                default_timeouts(),
            )
            .await;
            // Dropping would return at the deadline before lock release even starts.
            assert!(unlock_started.load(Ordering::SeqCst));
            result
        };
        let (result, ()) = tokio::join!(validation, unlock);
        assert!(
            matches!(result, Err(ExportError::Acquisition(e)) if e.kind == crate::catalog::AcquisitionErrorKind::Timeout)
        );
        sqlx::query("DELETE FROM snapshot_meta")
            .execute(&mut writer)
            .await
            .unwrap();
        writer.close().await.unwrap();
        let reader = SqliteConnection::connect_with(&options.read_only(true))
            .await
            .unwrap();
        assert!(validate_and_close(
            reader,
            tokio::time::Instant::now() + Duration::from_secs(5),
            default_timeouts()
        )
        .await
        .is_err());
        // Windows also requires every SQLite handle to be closed before deletion.
        fs::remove_file(&path).unwrap();
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn completed_files_replace_old_contents_without_sidecars() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog.sqlite");
        fs::write(&path, b"old bytes").unwrap();
        publish(&data("first"), &path).await.unwrap();
        publish(&data("second"), &path).await.unwrap();
        let request = TableRequest {
            schema: None,
            name: "users".into(),
        };
        let snapshot = super::super::SqliteCatalogProvider::new(&path)
            .acquire(std::slice::from_ref(&request))
            .await
            .unwrap();
        assert!(
            matches!(snapshot.lookup(&request), Lookup::Found(t) if t.columns[0].name == "second")
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn invalid_data_and_expired_deadlines_preserve_the_previous_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog.sqlite");
        fs::write(&path, b"old bytes").unwrap();
        let mut incomplete = data("new");
        incomplete.meta.counts[2] = 2;
        assert!(publish(&incomplete, &path).await.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old bytes");
        let deadline = tokio::time::Instant::now() - Duration::from_secs(1);
        assert!(
            publish_snapshot(&data("new"), &path, deadline, default_timeouts())
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(fs::read(&path).unwrap(), b"old bytes");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn concurrent_publications_are_complete_and_directories_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("catalog.sqlite");
        let first = data("first");
        let second = data("second");
        let (a, b) = tokio::join!(publish(&first, &path), publish(&second, &path));
        a.unwrap();
        b.unwrap();
        let request = TableRequest {
            schema: None,
            name: "users".into(),
        };
        let snapshot = super::super::SqliteCatalogProvider::new(&path)
            .acquire(std::slice::from_ref(&request))
            .await
            .unwrap();
        assert!(
            matches!(snapshot.lookup(&request),Lookup::Found(t) if ["first","second"].contains(&t.columns[0].name.as_str()))
        );
        assert!(publish(&first, temp.path()).await.is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
        assert!(publish(&first, &temp.path().join("absent/catalog.sqlite"))
            .await
            .is_err());
        assert!(!temp.path().join("absent").exists());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_destinations_preserve_the_target() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let path = temp.path().join("link");
        fs::write(&target, b"old bytes").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(publish(&data("new"), &path).await.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old bytes");
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    }
}
