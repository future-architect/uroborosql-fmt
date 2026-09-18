# uroborosql-lint (Beta)

Beta lint engine and rule/configuration crate for `uroborosql-lint`.

For CLI usage, see the [`uroborosql-lint-cli` README](../uroborosql-lint-cli/README.md).

## Configuration

The config file supports rule levels, file ignores, per-file overrides, and PostgreSQL catalog settings.

Example:

```json
{
  "rules": {
    "no-distinct": "error",
    "no-wildcard-projection": "warn"
  },
  "ignore": ["dist/**"],
  "overrides": [
    {
      "files": ["test/**/*.sql"],
      "rules": {
        "no-distinct": "off"
      }
    }
  ],
  "db": {
    "schemaProvider": "server",
    "host": "localhost",
    "user": "catalog_reader",
    "dbname": "app"
  }
}
```

### Rule Levels

Rule severities can be configured with:

- `off` / `"0"` / `0`
- `warn` / `warning` / `"1"` / `1`
- `error` / `"2"` / `2`

### `rules`

Configures the severity for each rule.

Unknown rule names are reported as configuration errors.

### `ignore`

Configures file globs to ignore.

### `overrides`

Configures per-file rule settings.

Each override must have:

- `files`: glob patterns to match target files
- `rules`: rule settings applied to matched files

### `db`

Configures how schema information should be loaded for rules that need database metadata.

The async API and CLI use `server` settings to check references against PostgreSQL
14–18. `host`, `user`, and `dbname` are required; `port` defaults to 5432.
`password` is optional and otherwise uses `PGPASSWORD`. pgpass files are not read.
The connection's effective search path is used; `PGOPTIONS` can configure it.

```json
{
  "db": {
    "schemaProvider": "server",
    "host": "localhost",
    "user": "catalog_reader",
    "dbname": "app",
    "tlsMode": "verify-full",
    "timeouts": {
      "connectMs": 5000,
      "queryMs": 5000,
      "acquisitionMs": 10000
    }
  }
}
```

`tlsMode` accepts `verify-full` (default), `verify-ca`, `require`, and `disable`.
The explicit/default mode overrides `PGSSLMODE`. The first two verify the server
certificate; `verify-full` also verifies the hostname. `require` encrypts without
certificate verification; `disable` is plaintext. Existing SQLx certificate
settings such as `PGSSLROOTCERT` remain available.

Timeouts are positive integer milliseconds. Omitted fields keep the defaults
shown above. The overall acquisition limit includes connection setup and all
queries, rather than resetting for each table.

`schemaProvider: file` reads a validated SQLite snapshot at `path`, relative to
the config directory. Enable `sqlite-catalog` to use `SqliteCatalogProvider` without
PostgreSQL or TLS dependencies. The reader opens an existing file read-only,
validates the complete format-v1 catalog in one transaction, and uses its stored
search path and schema USAGE decisions. Missing or invalid files fail acquisition;
they never create a file or fall back to a server. Only PostgreSQL 14–18 snapshots
are supported. See the [CLI export workflow](../uroborosql-lint-cli/README.md#export-and-offline-catalog-checks)
for creating and refreshing snapshots.

## Library API

`Linter::run` remains synchronous and runs CST rules only. `run_async` also runs
`no-unknown-reference` using a caller-selected `CatalogProvider` and returns
`LintResult { diagnostics, catalog }`. The catalog report preserves each
statement's span, status, exclusion reason, and recovered-source deferral.
Acquisition failures retain the CST diagnostics; parse failures still return
`LintError`. A completed traversal does not imply a recovered source was checked.

Typical calling code inside an existing async runtime:

```rust,ignore
let provider = resolved_config.catalog_provider();
let result = Linter::new()
    .run_async(sql, &resolved_config, provider.as_deref())
    .await?;
```

The library does not create a runtime. Enable `postgres-catalog` when constructing
a configured PostgreSQL provider; enable `sqlite-catalog` for file snapshots. The CLI
enables both by default. The default library build
still supports in-memory/custom providers without SQLx. Passing a provider
explicitly selects it regardless of `resolved_config.db`; passing `None` skips
catalog analysis. Provider construction performs no I/O.

There is no acquisition when the source is unconfigured, the catalog rule is off
(including file overrides), or syntax preparation produces no table requests.
SQL disable comments suppress diagnostics, not acquisition. A configured provider
missing from the build fails only when acquisition is needed.

## Path Resolution

- `ignore` and `overrides.files` are resolved relative to the current working directory
- `db.path` is resolved relative to the directory containing the loaded config file

## Directive Comments

Line comment directives can suppress specific lint rules directly in SQL.

Supported directives:

- `-- uroborosql-lint-disable <rules>`
- `-- uroborosql-lint-disable-next-line <rules>`

Rules must be comma-separated canonical rule names such as `no-distinct` or `no-wildcard-projection`.

Examples:

```sql
-- uroborosql-lint-disable no-distinct
SELECT DISTINCT id FROM users;
```

```sql
-- uroborosql-lint-disable-next-line no-distinct, no-wildcard-projection
SELECT DISTINCT * FROM users;
```

Behavior:

- `disable-next-line` suppresses diagnostics whose start position is on the next physical line only
- `disable` suppresses rules for the whole file, but only when it appears in the file head comment section
- The file head comment section is the leading sequence of blank lines and line comments
- A block comment ends that file head section, so any later `disable` directive is ignored
- Unknown rule names in directives produce an `invalid-lint-directive` warning on the comment, while known rules in the same directive still apply
- Missing rule names, empty comma-separated elements, and trailing commas also produce an `invalid-lint-directive` warning
