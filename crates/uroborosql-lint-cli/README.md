# uroborosql-lint-cli (Beta)

Beta CLI for `uroborosql-lint`.

## Install

```sh
cargo install --git https://github.com/future-architect/uroborosql-fmt uroborosql-lint-cli
```

The installed binary name is `uroborosql-lint`.

## Getting Started

SQL lint requires a lint config file. `export-catalog` uses connection arguments only.

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

## PostgreSQL catalog checks

Configure `db.schemaProvider: server` as described in the lint engine README,
then run the same lint command. Supported single-table SELECT references are
checked against PostgreSQL; the SQL being linted is never executed.

SQL diagnostics retain the existing stdout format. A separate stderr summary
reports completed, excluded, and failed statements, including why statements
were excluded. Unconfigured or disabled catalog checks print a short skip reason.
Unsupported SQL alone is not an execution error. Recovered sources whose
existence cannot be checked are reported as deferred.

If catalog acquisition fails, available CST diagnostics are still printed and
the exit code is 2, even with `--fail-level none`. There is no database access
when catalog checking is unconfigured, disabled by config, or has no table
requests. File-backed checks follow the same rules and never connect to PostgreSQL.

## Export and offline catalog checks

On a machine that can reach PostgreSQL 14–18, export the catalog:

```sh
uroborosql-lint export-catalog --host db.example.com --user catalog_reader --dbname app --output catalog.sqlite
```

Set `PGPASSWORD` if authentication requires a password. Export does not read or
require a lint config or SQL input. `--port` defaults to 5432; `--tls-mode` defaults
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

## Limitations

`uroborosql-lint` is in beta and currently has the following limitations:

- It accepts exactly one SQL file per invocation (`<INPUT>`).
  - Directory, glob, and multiple-file arguments are not supported yet.
  - Standard input is not supported; pass a file path instead.

To lint multiple files, invoke the command once per file (for example, from a shell loop or your build tool).
