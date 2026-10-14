# no-function-on-column-in-join-or-where

Reports functions applied to columns in `JOIN` or `WHERE` conditions. Wrapping a column in
a function can prevent the database from using an index on it.

Default severity: `warning`

## Examples

```sql
-- NG
SELECT * FROM users WHERE lower(users.name) = 'foo';
SELECT * FROM t1 JOIN t2 ON lower(t1.name) = t2.name;

-- OK
SELECT * FROM users WHERE users.id = 1;
SELECT func(col) FROM tbl;
```

Source: [SQL coding standards (PostgreSQL)](https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html) (Japanese)
