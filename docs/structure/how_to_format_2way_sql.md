# How to format 2way-sql

![2way-sql_example](../../images/2way_sql.png)

If the input SQL contains an IF branch, it is judged to be 2-way-sql and formatted in the following flow.

1. Generate multiple SQLs that can occur, taking into account all branches of the input SQL.
2. Format all SQL.
3. Merge all formatting results and output
