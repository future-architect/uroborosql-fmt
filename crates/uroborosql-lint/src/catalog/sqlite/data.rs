use super::{invalid, read_error, AcquisitionError};
use sqlx::{sqlite::SqliteRow, Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct SnapshotData {
    pub meta: Meta,
    pub namespaces: BTreeMap<u32, String>,
    pub relations: BTreeMap<u32, Relation>,
    pub attributes: BTreeMap<(u32, i16), Attribute>,
    pub access: BTreeMap<u32, bool>,
    pub path: BTreeMap<i64, u32>,
}
// Source identity is validated on read and retained for the optional exporter.
#[cfg_attr(not(feature = "postgres-catalog"), allow(dead_code))]
#[derive(Default)]
pub(super) struct Meta {
    pub version: i64,
    pub database: String,
    pub session_user: String,
    pub current_user: String,
    pub captured_at: String,
    pub counts: [i64; 4],
}
pub(super) struct Relation {
    pub namespace: u32,
    pub name: String,
    pub kind: String,
    pub count: i64,
}
pub(super) struct Attribute {
    pub name: String,
    pub dropped: bool,
}

// Checking typeof explicitly prevents SQLite affinity/coercion from accepting malformed files.
async fn rows(
    connection: &mut SqliteConnection,
    table: &str,
    columns: &[&str],
) -> Result<Vec<SqliteRow>, AcquisitionError> {
    let fields = columns
        .iter()
        .map(|c| format!("{c}, typeof({c}) AS type_{c}"))
        .collect::<Vec<_>>()
        .join(",");
    // Identifiers come only from the fixed format-v1 column lists below, never a file or caller.
    sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {fields} FROM {table}")))
        .fetch_all(connection)
        .await
        .map_err(read_error)
}
fn integer(row: &SqliteRow, field: &str) -> Result<i64, AcquisitionError> {
    let t: String = row
        .try_get(format!("type_{field}").as_str())
        .map_err(read_error)?;
    if t != "integer" {
        return Err(invalid());
    }
    row.try_get(field).map_err(read_error)
}
fn string(row: &SqliteRow, field: &str) -> Result<String, AcquisitionError> {
    let t: String = row
        .try_get(format!("type_{field}").as_str())
        .map_err(read_error)?;
    if t != "text" {
        return Err(invalid());
    }
    row.try_get(field).map_err(read_error)
}
fn oid(row: &SqliteRow, field: &str) -> Result<u32, AcquisitionError> {
    integer(row, field)?.try_into().map_err(|_| invalid())
}
fn boolean(row: &SqliteRow, field: &str) -> Result<bool, AcquisitionError> {
    match integer(row, field)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid()),
    }
}
fn insert<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, value: V) -> Result<(), AcquisitionError> {
    if map.insert(key, value).is_some() {
        Err(invalid())
    } else {
        Ok(())
    }
}

pub(super) async fn read_validated(
    connection: &mut SqliteConnection,
) -> Result<SnapshotData, AcquisitionError> {
    let integrity: Vec<(String,)> = sqlx::query_as("PRAGMA integrity_check")
        .fetch_all(&mut *connection)
        .await
        .map_err(read_error)?;
    if integrity != [("ok".into(),)] {
        return Err(invalid());
    }
    if !sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *connection)
        .await
        .map_err(read_error)?
        .is_empty()
    {
        return Err(invalid());
    }
    // Require real tables, not views which can manufacture apparent complete catalogs.
    for table in [
        "pg_namespace",
        "pg_class",
        "pg_attribute",
        "snapshot_meta",
        "snapshot_schema_access",
        "snapshot_search_path",
    ] {
        let kind: Option<(String,)> =
            sqlx::query_as("SELECT type FROM sqlite_schema WHERE name = ? COLLATE BINARY")
                .bind(table)
                .fetch_optional(&mut *connection)
                .await
                .map_err(read_error)?;
        if kind != Some(("table".into(),)) {
            return Err(invalid());
        }
    }
    let mut data = SnapshotData::default();
    let meta = rows(
        connection,
        "snapshot_meta",
        &[
            "id",
            "format_version",
            "server_version_num",
            "database_name",
            "session_user",
            "current_user",
            "captured_at",
            "scope",
            "complete",
            "namespace_count",
            "relation_count",
            "attribute_count",
            "search_path_count",
        ],
    )
    .await?;
    if meta.len() != 1 {
        return Err(invalid());
    }
    let row = &meta[0];
    if integer(row, "id")? != 1
        || integer(row, "format_version")? != 1
        || integer(row, "complete")? != 1
        || string(row, "scope")? != "database_catalog"
    {
        return Err(invalid());
    }
    data.meta = Meta {
        version: integer(row, "server_version_num")?,
        database: string(row, "database_name")?,
        session_user: string(row, "session_user")?,
        current_user: string(row, "current_user")?,
        captured_at: string(row, "captured_at")?,
        counts: [
            integer(row, "namespace_count")?,
            integer(row, "relation_count")?,
            integer(row, "attribute_count")?,
            integer(row, "search_path_count")?,
        ],
    };
    for row in rows(connection, "pg_namespace", &["oid", "nspname"]).await? {
        insert(
            &mut data.namespaces,
            oid(&row, "oid")?,
            string(&row, "nspname")?,
        )?;
    }
    for row in rows(
        connection,
        "pg_class",
        &["oid", "relnamespace", "relname", "relkind", "relnatts"],
    )
    .await?
    {
        insert(
            &mut data.relations,
            oid(&row, "oid")?,
            Relation {
                namespace: oid(&row, "relnamespace")?,
                name: string(&row, "relname")?,
                kind: string(&row, "relkind")?,
                count: integer(&row, "relnatts")?,
            },
        )?;
    }
    for row in rows(
        connection,
        "pg_attribute",
        &["attrelid", "attnum", "attname", "attisdropped"],
    )
    .await?
    {
        let number = integer(&row, "attnum")?.try_into().map_err(|_| invalid())?;
        insert(
            &mut data.attributes,
            (oid(&row, "attrelid")?, number),
            Attribute {
                name: string(&row, "attname")?,
                dropped: boolean(&row, "attisdropped")?,
            },
        )?;
    }
    for row in rows(
        connection,
        "snapshot_schema_access",
        &["nspoid", "usage_allowed"],
    )
    .await?
    {
        insert(
            &mut data.access,
            oid(&row, "nspoid")?,
            boolean(&row, "usage_allowed")?,
        )?;
    }
    for row in rows(connection, "snapshot_search_path", &["position", "nspoid"]).await? {
        insert(
            &mut data.path,
            integer(&row, "position")?,
            oid(&row, "nspoid")?,
        )?;
    }
    data.validate()?;
    Ok(data)
}

impl SnapshotData {
    pub fn validate(&self) -> Result<(), AcquisitionError> {
        if !(14..=18).contains(&(self.meta.version / 10000)) {
            return Err(
                invalid().with_detail(crate::catalog::AcquisitionDetail::UnsupportedServerVersion)
            );
        }
        if !utc_timestamp(&self.meta.captured_at)
            || self.meta.counts
                != [
                    self.namespaces.len() as i64,
                    self.relations.len() as i64,
                    self.attributes.len() as i64,
                    self.path.len() as i64,
                ]
        {
            return Err(invalid());
        }
        if self.namespaces.values().collect::<BTreeSet<_>>().len() != self.namespaces.len()
            || self.namespaces.keys().ne(self.access.keys())
        {
            return Err(invalid());
        }
        let mut relation_names = BTreeSet::new();
        for (oid, relation) in &self.relations {
            if !self.namespaces.contains_key(&relation.namespace)
                || relation.kind.chars().count() != 1
                || !(0..=i16::MAX as i64).contains(&relation.count)
                || !relation_names.insert((relation.namespace, &relation.name))
            {
                return Err(invalid());
            }
            let mut expected = 1i64;
            let mut names = BTreeSet::new();
            for ((_, number), attribute) in
                self.attributes.range((*oid, i16::MIN)..=(*oid, i16::MAX))
            {
                if *number == 0 || (*number < 0 && attribute.dropped) {
                    return Err(invalid());
                }
                if *number > 0 {
                    if i64::from(*number) != expected {
                        return Err(invalid());
                    }
                    expected += 1;
                }
                if !attribute.dropped && !names.insert(&attribute.name) {
                    return Err(invalid());
                }
            }
            if expected != relation.count + 1 {
                return Err(invalid());
            }
        }
        if self
            .attributes
            .keys()
            .any(|(oid, _)| !self.relations.contains_key(oid))
        {
            return Err(invalid());
        }
        let mut path_oids = BTreeSet::new();
        for ((position, oid), expected) in self.path.iter().zip(1i64..) {
            if *position != expected
                || self.access.get(oid) != Some(&true)
                || !path_oids.insert(oid)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn utc_timestamp(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b.last() != Some(&b'Z')
    {
        return false;
    }
    let number = |start, end| {
        value
            .get(start..end)
            .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<u32>().ok())
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    ) else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    day >= 1
        && day <= days
        && hour < 24
        && minute < 60
        && second < 60
        && (b.len() == 20
            || (b.len() > 21 && b[19] == b'.' && b[20..b.len() - 1].iter().all(u8::is_ascii_digit)))
}
