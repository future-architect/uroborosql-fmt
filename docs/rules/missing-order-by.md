# missing-order-by

Reports order-dependent aggregate functions called without `ORDER BY`. The order in which rows
reach the aggregate is undefined, so the result can change between runs.

Default severity: `warning`

Target functions: `array_agg`, `string_agg`, `json_agg`, `jsonb_agg`, `xmlagg`

Aggregates used with `OVER` or `WITHIN GROUP` are not checked.

## Examples

```sql
-- NG
SELECT json_agg(item_name) FROM items;
SELECT category, array_agg(DISTINCT item_name) FROM items GROUP BY category;
SELECT xmlagg(xmlelement(name item, item_name)) FROM items WHERE price > 0;

-- OK
SELECT json_agg(item_name ORDER BY item_name) FROM items;
SELECT category, array_agg(DISTINCT item_name ORDER BY item_name) FROM items GROUP BY category;
SELECT xmlagg(xmlelement(name item, item_name) ORDER BY id) FROM items WHERE price > 0;
SELECT sum(price) FROM items;
```

Source: [PostgreSQL: Aggregate Functions](https://www.postgresql.org/docs/current/functions-aggregate.html)
