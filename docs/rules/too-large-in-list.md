# too-large-in-list

Reports `IN` lists with more than 100 elements.

Default severity: `warning`

## Examples

```sql
-- NG: IN list has 101 elements (limit: 100)
SELECT * FROM users WHERE id IN (1, 2, 3, ..., 101);

-- OK
SELECT * FROM users WHERE id IN (SELECT id FROM admins);
```

The limit is fixed and cannot be configured.

Source: [SQL coding standards (PostgreSQL): IN](https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#in-%E5%8F%A5-1) (Japanese)
