# convert_double_colon_cast

Convert casts by `X::type` to the form `CAST(X AS type)`.

## Options

- `true` (default): Convert casts by `X::type` to the form `CAST(X AS type)`.
- `false` : Do not convert casts by `X::type`.

## Example

before:

```sql
SELECT
	''::JSONB
FROM
 	TBL
```

result:

```sql
SELECT
	CAST(''	AS	JSONB)
FROM
	TBL
```
