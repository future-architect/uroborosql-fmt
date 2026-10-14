# missing-two-way-sample

Reports 2way-SQL bind parameters without a sample value, such as `/*param*/`.
Without a sample value, the SQL cannot be run as is in an SQL client.

Default severity: `warning`

## Examples

```sql
-- NG
SELECT * FROM t WHERE col = /*col*/;

-- OK
SELECT * FROM t WHERE col = /*col*/1;
SELECT /*param1*/'' AS name FROM t;
```

Replacement and embedded parameters such as `/*#table*/` and `/*$table*/` are not reported.

Source: [uroboroSQL: bind parameters](https://future-architect.github.io/uroborosql-doc/background/#バインドパラメータ) (Japanese)
