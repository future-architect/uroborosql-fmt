# no-distinct

Reports `DISTINCT`.

Default severity: `warning`

## Examples

```sql
-- NG
SELECT DISTINCT id FROM users;

-- OK
SELECT id FROM users;
```

Source: [SQL coding standards (PostgreSQL): DISTINCT](https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#distinct-%E5%8F%A5) (Japanese)
