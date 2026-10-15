# no-unknown-reference

Reports tables and columns that do not exist in the database catalog.

Default severity: `error`

This rule needs a `db` section in the lint config. Without one, the rule does nothing and
no database is accessed. With one, the rule is enabled by default.

## Examples

With a catalog where `users` has the columns `id` and `name`:

```sql
-- NG: Table `user_info` does not exist.
SELECT id FROM user_info;

-- NG: Column `email` does not exist.
SELECT email FROM users;

-- OK
SELECT id, name FROM users u WHERE u.id = 1;
```

## Supported SQL

> [!NOTE]
> **Beta limitation:** Only a single-table `SELECT` is checked:
>
> ```
> SELECT [DISTINCT] <columns> FROM <one table> [alias]
>   [WHERE ...] [LIMIT ...] [OFFSET ...] [FOR UPDATE ...]
> ```
>
> Any other statement is skipped without a diagnostic. This includes `JOIN`, subqueries, `WITH`,
> `UNION`, `GROUP BY`, `ORDER BY`, and `INSERT` / `UPDATE` / `DELETE`. No diagnostic does not
> mean that every reference in the file was checked. The CLI (stderr) and the language server
> (server log) list each skipped statement with its position.

## Catalog sources

- `"schemaProvider": "server"`: connects to PostgreSQL 14–18 and reads the catalog.
- `"schemaProvider": "file"`: reads a SQLite snapshot created with
  `uroborosql-lint export-catalog`.

See the [lint CLI README](../../crates/uroborosql-lint-cli/README.md#catalog-checks) for setup.
The SQL being linted is never executed.
