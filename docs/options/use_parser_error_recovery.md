# use_parser_error_recovery

Enable error recovery in the SQL parser.

When this option is enabled, `postgresql-cst-parser` attempts to recover from certain syntax errors and continues formatting.

Internally, this feature relies on the `parse_2way` function of `postgresql-cst-parser`. You can refer to the original source code and documentation [here](https://github.com/future-architect/postgresql-cst-parser/blob/eeb064fb948c9496b9f108ffbe4621905a14a152/crates/postgresql-cst-parser/src/lib.rs#L103-L118).

## Options

- `true` (default): Enable parser error recovery and format even when the SQL has syntax errors.
- `false`: Disable parser error recovery. If a parse error occurs, the formatter returns an error without modifying the SQL.

## Example

With error recovery enabled, SQL like the following can be formatted without raising a parse error.

```sql
SELECT
,	/*foo*/
,	/*bar*/
FROM
,	/*#baz*/
,	/*#qux*/
WHERE
AND	1	=	1
AND	2	=	2
ORDER BY
,	1
,	2
;
```
