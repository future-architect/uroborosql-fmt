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

## Notes

The `AS` keyword is **not** removed in the following pattern, because PostgreSQL requires it; dropping it causes a syntax error.  
This behaviour is independent of the value of `remove_table_as_keyword`.

### Table function with column list and type annotations

before:

```sql
SELECT
	*
FROM
	unnest(a) AS (id int, name text);
```

result (AS remains):

```sql
SELECT
	*
FROM
	unnest(a) AS (id int, name text);
```

(If `AS` were omitted here, it would result in a parse error.)
