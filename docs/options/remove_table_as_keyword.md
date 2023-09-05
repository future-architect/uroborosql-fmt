# remove_table_as_keyword

Remove `AS` in table aliases.

## Options

- `true` (default): Remove table aliases `AS` if present.
- `false` : Do not remove table alias `AS`.

## Example

before:

```sql
SELECT
	COL1
FROM
	TABLE1 AS TBL1
```

result:

```sql
SELECT
	COL1
FROM
	TABLE1	TBL1
```
