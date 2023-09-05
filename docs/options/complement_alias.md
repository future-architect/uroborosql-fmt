# complement_alias

Complement aliases. Currently, column names are auto-completed with the same name.

## Options

- `true` (default): Complements column name aliases with the same name.
- `false` : Do not complement column name aliases.

## Example

before:

```sql
SELECT
	COL1
FROM
	TAB1
```

result:

```sql
SELECT
	COL1	AS	COL1
FROM
	TAB1
```
