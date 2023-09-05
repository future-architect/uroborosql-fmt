# trim_bind_param

Trim the contents of the [bind parameters](https://future-architect.github.io/uroborosql-doc/background/#%E3%83%8F%E3%82%99%E3%82%A4%E3%83%B3%E3%83%88%E3%82%99%E3%83%8F%E3%82%9A%E3%83%A9%E3%83%A1%E3%83%BC%E3%82%BF).

## Options

- `true` : Trim blanks before and after bind parameters.
- `false` (default): Do not trim blanks before and after bind parameters.

## Example

before:

```sql
SELECT
	*
FROM
	DEPARTMENT
WHERE
	DEPT_NO	=	/*     dept_no     */10
```

result:

```sql
SELECT
	*
FROM
	DEPARTMENT
WHERE
	DEPT_NO	=	/*dept_no*/10
```
