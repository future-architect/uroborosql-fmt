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

-- NG: Column `nmae` does not exist.
SELECT id FROM users ORDER BY nmae;

-- OK
SELECT id, name FROM users u WHERE u.id = 1;

-- OK: ORDER BY and GROUP BY can use output names.
SELECT id AS user_id FROM users ORDER BY user_id;
```

## Supported SQL

> [!NOTE]
> **Beta limitation:** Only `SELECT` statements are checked. Every clause is checked
> (`JOIN ... ON`, `WHERE`, `GROUP BY`, `HAVING`, `ORDER BY`, `DISTINCT ON`, and subqueries,
> including correlated ones), except values in `LIMIT` / `OFFSET`, the targets of `FOR UPDATE OF`,
> `WINDOW` definitions, `VALUES` lists, and `ORDER BY` after a parenthesized query.
>
> An unqualified column is reported only when every table visible to it was found in the
> catalog, so a column that may come from a CTE, a derived table, or a `UNION` / `INTERSECT` /
> `EXCEPT` result is not reported. A qualified column such as `u.email` is reported when the
> table behind `u` was found and has no such column. Three-part names such as
> `schema.table.column` are not checked.
>
> These FROM items are not supported: `LATERAL`, functions, `TABLESAMPLE`, `ONLY`, column
> aliases such as `AS u(a, b)`, and aliased joins (`(a JOIN b) AS j`, `JOIN ... USING (...) AS j`). A statement that has one is listed as not
> fully checked. The other references in the statement are still checked when they can be.
>
> If the input contains `INSERT` / `UPDATE` / `DELETE`, DDL, `SET`, `SELECT ... INTO`, or a
> function call such as `count(*)` or `lower(x)`, the whole file is skipped, because these can
> change the catalog. Built-in forms such as `COALESCE`, `CASE`, `CAST`, and `EXTRACT` do not.
>
> No diagnostic does not mean that every reference in the file was checked. The CLI (stderr)
> and the language server (server log) list each skipped statement with its position.

## Catalog sources

- `"schemaProvider": "server"`: connects to PostgreSQL 14–18 and reads the catalog.
- `"schemaProvider": "file"`: reads a SQLite snapshot created with
  `uroborosql-lint export-catalog`.

See the [lint CLI README](../../crates/uroborosql-lint-cli/README.md#catalog-checks) for setup.
The SQL being linted is never executed.
