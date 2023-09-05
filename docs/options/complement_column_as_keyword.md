# complement_column_as_keyword

Complement `AS` in column aliases.

## Options

- `true` (default): Complement column alias `AS` if it is omitted.
- `false` : Do not complement column alias `AS`.

## Example

before:

```sql
SELECT
	COLUMN1	COL1
FROM
	TBL
```

after:

```sql
SELECT
	COLUMN1	AS	COL1
FROM
	TBL
```
