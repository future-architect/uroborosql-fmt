WITH t AS (SELECT * FROM distributors WHERE active = true)
DELETE FROM distributors
USING t
WHERE distributors.id = t.id; 

WITH t1 AS NOT MATERIALIZED (SELECT * FROM tbl1 WHERE value > 0),
     t2 AS (SELECT * FROM tbl2 WHERE flag = true)
-- comment
-- comment
DELETE FROM tbl1
USING t1, t2
WHERE tbl1.id = t1.id AND t2.flag = true;
