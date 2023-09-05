# complement_sql_id

Complement [SQL ID](https://palette-doc.rtfa.as/coding-standards/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88uroboroSQL%EF%BC%89.html#sql-%E8%AD%98%E5%88%A5%E5%AD%90).

## Options

- `true` : Complement SQL ID.
- `false` (default): Do not complement SQL ID.

## Example

before:

```sql
SELECT
	COL1
FROM
	TBL1
```

result:

```sql
SELECT /* _SQL_ID_ */
	COL1
FROM
	TBL1
```
