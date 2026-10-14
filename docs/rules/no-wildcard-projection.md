# no-wildcard-projection

Reports wildcard projections such as `SELECT *`, `SELECT u.*`, and `RETURNING *`.
List the columns explicitly.

Default severity: `warning`

## Examples

```sql
-- NG
SELECT * FROM users;
SELECT u.* FROM users u;
INSERT INTO users(id) VALUES (1) RETURNING *;

-- OK
SELECT id, name FROM users;
SELECT count(*) FROM users;
```

Source: [SQL coding standards (PostgreSQL)](https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html) (Japanese)
