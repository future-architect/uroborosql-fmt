# keyword_case

Unify the case of keywords.

## Options

- `"upper"` (default): Unify keywords with upper cases.
- `"lower"`: Unify keywords with lower cases.
- `"preserve"`: Preserves the original case of keywords.

## Example

before:

```sql
Select
	*
FroM
	DEPARTMENT
wheRE
	DEPT_NO	=	10
```

### upper

```sql
SELECT
	*
FROM
	DEPARTMENT
WHERE
	DEPT_NO	=	10
```

### lower

```sql
select
	*
from
	DEPARTMENT
where
	DEPT_NO	=	10
```

### preserve

```sql
Select
	*
FroM
	DEPARTMENT
wheRE
	DEPT_NO	=	10
```
