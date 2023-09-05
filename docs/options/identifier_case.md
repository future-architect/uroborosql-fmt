# identifier_case

Unify the case of identifiers.

## Options

- `"upper"` (default): Unify identifiers with upper cases.
- `"lower"`: Unify identifiers with lower cases.
- `"preserve"`: Preserves the original case of identifiers.

## Example

before:

```sql
SELECT
	coL1
FROM
	Department
WHERE
	DEPT_no	=	10
```

### upper

```sql
SELECT
	COL1
FROM
	DEPARTMENT
WHERE
	DEPT_NO	=	10
```

### lower

```sql
SELECT
	col1
FROM
	department
WHERE
	dept_no	=	10
```

### preserve

```sql
SELECT
	coL1
FROM
	Department
WHERE
	DEPT_no	=	10
```
