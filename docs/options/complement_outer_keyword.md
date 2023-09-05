# complement_outer_keyword

Complement the optional OUTER. Specifically, `RIGHT OUTER JOIN`, `LEFT OUTER JOIN`, and `FULL OUTER JOIN`.

## Options

- `true` (default): If an optional `OUTER` is omitted, complement it.
- `false` : Do not complement `OUTER`.

## Example

before:

```sql
SELECT
	*
FROM
	T1
LEFT JOIN
	T2
ON
	T1.NUM	=	T2.NUM
```

result:

```sql
SELECT
	*
FROM
	T1
LEFT OUTER JOIN
	T2
ON
	T1.NUM	=	T2.NUM
```
