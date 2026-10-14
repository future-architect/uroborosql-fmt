# no-union-distinct

Reports `UNION` and `UNION DISTINCT`. `UNION` without `ALL` removes duplicate rows,
which is the same as `UNION DISTINCT`. Use `UNION ALL` when duplicates cannot occur or
are acceptable.

Default severity: `warning`

## Examples

```sql
-- NG
SELECT 1 UNION SELECT 2;
SELECT 1 UNION DISTINCT SELECT 2;

-- OK
SELECT 1 UNION ALL SELECT 2;
```

Source: [SQL coding standards (PostgreSQL): UNION](https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#union-%E5%8F%A5) (Japanese)
