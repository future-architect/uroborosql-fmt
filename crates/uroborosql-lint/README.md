# uroborosql-lint

The CLI binary name is `uroborosql-lint`.

## Usage

```bash
uroborosql-lint [OPTIONS] <INPUT>
```

Examples:

```bash
uroborosql-lint query.sql
uroborosql-lint --config .uroborosqllintrc.json query.sql
uroborosql-lint --fail-level warning query.sql
```

## Exit Codes

- `0`: lint succeeded and no diagnostics at or above `--fail-level` were found
- `1`: lint succeeded and at least one diagnostic at or above `--fail-level` was found
- `2`: lint could not complete because of a usage or execution failure such as invalid CLI arguments, invalid config, I/O failure, or SQL parse failure

## Fail Level

Use `--fail-level <none|info|warning|error>` to control which diagnostics cause a non-zero exit code.

- Default: `error`
- `info` currently behaves the same as `warning` because the implemented diagnostics are `warning` or `error` today; it exists so the CLI can stay aligned if `info` diagnostics are added later
- `warning` is useful for CI when warnings, including lint directive warnings, should fail the run
- `none` keeps diagnostics visible without failing the process

## Configuration

The default config file name is `.uroborosqllintrc.json`.

## Directive Comments

Line comment directives can suppress specific lint rules directly in SQL.

Supported directives:

- `-- uroborosql-lint-disable <rules>`
- `-- uroborosql-lint-disable-next-line <rules>`

Rules must be comma-separated canonical rule names such as `no-distinct` or `no-wildcard-projection`.

Example:

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
    "schemaProvider": "file",
    "path": "schema/schema.sql"
  }
}
```

### `rules`

Configures the severity for each rule.

Unknown rule names are reported as configuration errors.

Supported values:

- `off` / `"0"` / `0`
- `warn` / `warning` / `"1"` / `1`
- `error` / `"2"` / `2`

### `ignore`

Configures file globs to ignore.

### `overrides`

Configures per-file rule settings.

Each override must have:

- `files`: glob patterns to match target files
- `rules`: rule settings applied to matched files

### `db`

Configures how schema information should be loaded for rules that need database metadata.

This setting is reserved for schema-aware rules.
It is not used by the currently implemented rules yet.

Supported values are:

- `file`
  - Loads schema information from a file
  - `path` is required
- `server`
  - Loads schema information from a PostgreSQL server
  - `host`, `user`, and `dbname` are required
  - `port` and `password` are optional

Examples:

```json
{
  "db": {
    "schemaProvider": "file",
    "path": "schema/schema.sql"
  }
}
```

```json
{
  "db": {
    "schemaProvider": "server",
    "host": "localhost",
    "port": 5432,
    "user": "postgres",
    "password": "secret",
    "dbname": "app"
  }
}
```

## Path Resolution

- `ignore` and `overrides.files` are resolved relative to the current working directory
- `db.path` is resolved relative to the directory containing the loaded config file
