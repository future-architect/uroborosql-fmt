# OptionName

Remove redundant parentheses.

## Options

- `true` (default): Remove redundant parentheses.
- `false` : Preserve redundant parentheses.

## Example

before:

```sql
SELECT
	A
FROM
	B
WHERE
	(((1	=	1)))
AND	(
		((A	=	B))
	OR	(A)			=	(((42)))
	)
```

result:

```sql
SELECT
	A
FROM
	B
WHERE
	(1	=	1)
AND	(
		(A	=	B)
	OR	(A)			=	(42)
	)
```
