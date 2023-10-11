# unify_not_equal

Convert comparison operator `<>` to `!=`.

## Options

- `true` (default): Convert `<>` to `!=`.
- `false` : Do not convert `<>` to `!=`.

## Example

before:

```sql
SELECT
	*
FROM
	STUDENTS
WHERE
	STUDENT_ID	<>	2
```

result:

```sql
SELECT
	*
FROM
	STUDENTS
WHERE
	STUDENT_ID	!=	2
```
