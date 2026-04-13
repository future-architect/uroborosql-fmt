# uroborosql-lint

## Configuration

The default config file name is `.uroborosqllintrc.json`.

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
