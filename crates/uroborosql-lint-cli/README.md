# uroborosql-lint-cli (Beta)

Beta CLI for `uroborosql-lint`.

## Install

### Prebuilt binary

Download the binary for your platform from the latest `uroborosql-lint-cli-v*` release on the
[Releases page](https://github.com/future-architect/uroborosql-fmt/releases):

| Platform | File |
| --- | --- |
| Windows (x64) | `uroborosql-lint-x86_64-pc-windows-msvc.exe` |
| Windows (ARM64) | `uroborosql-lint-aarch64-pc-windows-msvc.exe` |
| macOS (Apple silicon) | `uroborosql-lint-aarch64-apple-darwin` |
| macOS (Intel) | `uroborosql-lint-x86_64-apple-darwin` |
| Linux (x64) | `uroborosql-lint-x86_64-unknown-linux-gnu` |

Each file is a standalone executable. Rename it to `uroborosql-lint` (`uroborosql-lint.exe`
on Windows) and put it in a directory on your `PATH`. On macOS and Linux, also run
`chmod +x uroborosql-lint`.

### Build with cargo

```sh
cargo install --git https://github.com/future-architect/uroborosql-fmt uroborosql-lint-cli
```

The installed binary name is `uroborosql-lint`. Run `uroborosql-lint --help` to check the
installation.

## Getting Started

Linting SQL requires a lint config file (`.uroborosqllintrc.json`). The `export-catalog`
subcommand exports a catalog snapshot for offline checks and does not use the lint config; see
[Export and offline catalog checks](#export-and-offline-catalog-checks).

### 1. Create a starter config file

```bash
uroborosql-lint --init
```

This creates `.uroborosqllintrc.json` in the current working directory.

### 2. Run lint

```bash
uroborosql-lint query.sql
```

You can also create `.uroborosqllintrc.json` manually or pass a config path with `--config`.

For config file structure, rule settings, and directive details, see the
[`uroborosql-lint` README](../uroborosql-lint/README.md).

## Usage

```bash
uroborosql-lint [OPTIONS] <INPUT>
uroborosql-lint --init
uroborosql-lint export-catalog --host HOST --user USER --dbname DB [--output FILE]
```

Examples:

```bash
uroborosql-lint query.sql
uroborosql-lint --config .uroborosqllintrc.json query.sql
uroborosql-lint --fail-level warning query.sql
uroborosql-lint --init
```

If no lint config can be resolved, the CLI exits with an execution error and prints guidance to
create one.

### Init

Use `uroborosql-lint --init` to create a starter `.uroborosqllintrc.json` in the current working
directory. If the file already exists, the command fails without overwriting it.

### Exit Codes

- `0`: lint succeeded and no diagnostics at or above `--fail-level` were found
- `1`: lint succeeded and at least one diagnostic at or above `--fail-level` was found
- `2`: lint could not complete because of a usage or execution failure such as missing config, invalid CLI arguments, invalid config, I/O failure, SQL parse failure, or catalog acquisition failure

### Fail Level

Use `--fail-level <none|info|warning|error>` to control which diagnostics cause a non-zero exit code.

- Default: `error`
- `info` currently behaves the same as `warning` because the implemented diagnostics are `warning` or `error` today; it exists so the CLI can stay aligned if `info` diagnostics are added later
- `warning` is useful for CI when warnings, including lint directive warnings, should fail the run
- `none` keeps diagnostics visible without diagnostic-threshold failure; execution failures still return 2

## Catalog checks

Lint rules are either rules that check the SQL alone or rules that also use the database
catalog. The [rule list](../uroborosql-lint/README.md#rules) shows which rules use the catalog.
Rules that use the catalog run only when the lint config has a `db` section; without one, no
database is read. There are two ways to provide the catalog:

- Export the catalog to a SQLite file and lint without a database connection
  (`"schemaProvider": "file"`). This is recommended for regular use and CI, because linting
  needs no database connection or credentials. See
  [Export and offline catalog checks](#export-and-offline-catalog-checks).
- Connect to PostgreSQL directly (`"schemaProvider": "server"`). This is useful for trying
  catalog checks out; every lint run connects to the database. See
  [PostgreSQL catalog checks](#postgresql-catalog-checks).

SQL diagnostics retain the existing stdout format. A separate stderr summary
reports completed, excluded, and failed statements, including why statements
were excluded. Unconfigured or disabled catalog checks print a short skip reason.
Unsupported SQL alone is not an execution error. Recovered sources whose
existence cannot be checked are reported as deferred.

If catalog acquisition fails, available CST diagnostics are still printed and
the exit code is 2, even with `--fail-level none`. There is no database access
when catalog checking is unconfigured, disabled by config, or has no table
requests. File-backed checks follow the same rules and never connect to PostgreSQL.

Only single-table `SELECT` statements are checked for now. See [Limitations](#limitations).

### Export and offline catalog checks

On a machine that can reach PostgreSQL 14–18, export the catalog:

```sh
uroborosql-lint export-catalog --host db.example.com --user catalog_reader --dbname app --output catalog.sqlite
```

Set `PGPASSWORD` if authentication requires a password. Export does not read or
require a lint config or SQL input; the PostgreSQL connection settings are passed as
command-line arguments. `--port` defaults to 5432; `--tls-mode` defaults
to `verify-full` and also accepts `verify-ca`, `require`, and `disable`.
Explicit arguments and defaults take precedence over `PGHOST`, `PGUSER`,
`PGDATABASE`, `PGPORT`, and `PGSSLMODE`. Password, `PGOPTIONS`, and certificate
environment variables follow the PostgreSQL provider's rules; pgpass is not read.

The snapshot preserves the effective search path and schema USAGE decisions of
the export session. For example, set `PGOPTIONS='-c search_path=app,public'` in a
POSIX shell, or `$env:PGOPTIONS = '-c search_path=app,public'` in PowerShell.
It contains object names, database and user names, and acquisition metadata, but
no application data rows, host name, password, or connection string.

Save this as `offline-lint.json` beside the snapshot, then copy both to the offline environment:

```json
{
  "db": { "schemaProvider": "file", "path": "catalog.sqlite" },
  "rules": { "no-unknown-reference": "error" }
}
```

```sh
uroborosql-lint query.sql --config offline-lint.json
```

The file path is relative to the config directory. PostgreSQL and its credentials
are unnecessary for offline lint. Missing, corrupt, incomplete, or incompatible
snapshots fail acquisition with exit code 2 while retaining other lint diagnostics.
Snapshots do not refresh automatically; export again after relevant DDL changes.
Stored privileges describe the export session, not the current CI user.

`--output` is relative to the working directory. Omit it for a UTC name such as
`catalog-20260918T111500Z.sqlite`. Both explicit and default names overwrite an
existing regular file atomically after writing, validating, and closing a temporary
file in the same directory. Failures preserve the previous file. Directories and
symlinks are rejected, and parent directories are not created. No WAL sidecar is
needed. Concurrent exports use the last successful replacement; there is no lock
ordering or collision suffix. A same-named SQL file can be linted as `./export-catalog`.

Export accepts `--connect-timeout-ms`, `--query-timeout-ms`, and
`--acquisition-timeout-ms`, each a positive integer. Omitted values are 5000,
30000, and 120000 ms respectively. The overall deadline covers acquisition,
writing, and validation up to publication; timeout never publishes later.
Local file operations and closing SQLite workers can delay cleanup beyond the
deadline. Once atomic replacement starts, its result determines success.
Export returns 0 on success and 2 on any failure, reports the result and output
path to stderr, and leaves stdout empty.

### PostgreSQL catalog checks

Configure `db.schemaProvider: server` as described in the lint engine README,
then run the same lint command. Supported single-table SELECT references are
checked against PostgreSQL; the SQL being linted is never executed.

## Limitations

`uroborosql-lint` is in beta and currently has the following limitations:

- It accepts exactly one SQL file per invocation (`<INPUT>`).
  - Directory, glob, and multiple-file arguments are not supported yet.
  - Standard input is not supported; pass a file path instead.
  - To lint multiple files, invoke the command once per file (for example, from a shell loop or your build tool).
- Catalog checks cover only single-table `SELECT` statements:

  ```
  SELECT [DISTINCT] <columns> FROM <one table> [alias]
    [WHERE ...] [LIMIT ...] [OFFSET ...] [FOR UPDATE ...]
  ```

  Other statements, including `JOIN`, subqueries, `WITH`, `UNION`, `GROUP BY`, `ORDER BY`,
  and `INSERT` / `UPDATE` / `DELETE`, are skipped without a diagnostic. The stderr summary
  lists each skipped statement with its position:

  ```
  query.sql: catalog: 1 checked, 2 not checked, 0 failed
  query.sql:3:1: not checked against the catalog (unsupported syntax)
  query.sql:10:1: not checked against the catalog (unsupported syntax)
  ```
