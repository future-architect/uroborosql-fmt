# no-not-in

Reports `NOT IN`. `NOT IN` returns no rows when the list or subquery contains `NULL`;
use `NOT EXISTS` instead.

Default severity: `warning`

## Examples

```sql
-- NG
SELECT value FROM users WHERE id NOT IN (1, 2);
SELECT value FROM users WHERE id NOT IN (SELECT id FROM admins);

-- OK
SELECT value FROM users WHERE id IN (1, 2);
SELECT value FROM users u WHERE NOT EXISTS (SELECT 1 FROM admins a WHERE a.id = u.id);
```

Source: [SQL coding standards (PostgreSQL): NOT IN](https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#not-in-%E5%8F%A5) (Japanese)
