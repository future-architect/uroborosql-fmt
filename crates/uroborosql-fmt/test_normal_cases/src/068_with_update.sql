WITH t AS (SELECT * FROM distributors WHERE active = true)
UPDATE distributors
SET col = 1
FROM t
WHERE distributors.id = t.id;

WITH t1 AS NOT MATERIALIZED (SELECT * FROM tbl1 WHERE value > 0),
     t2 AS (SELECT * FROM tbl2 WHERE flag = true)
-- comment
-- comment
UPDATE tbl1
SET col = 1
FROM t1, t2
WHERE tbl1.id = t1.id AND t2.flag = true;
