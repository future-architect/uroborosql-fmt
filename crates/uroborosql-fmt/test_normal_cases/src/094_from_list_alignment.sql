SELECT *
FROM table1 t1, -- regular table with alias
     (SELECT id, name FROM users WHERE active = true) u, -- subquery with alias
     table2 t2 LEFT JOIN table3 t3 ON t2.id = t3.id -- joined table condition 1
     and t2.name = t3.name, -- joined table condition 2
     (table4 t4 INNER JOIN table5 t5 ON t4.ref_id = t5.id) joined_tables -- parenthesized join with alias
WHERE t1.user_id = u.id;
