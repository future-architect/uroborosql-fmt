-- simple with
WITH t AS (SELECT 1) SELECT * FROM t;

-- special name
WITH time AS (SELECT 1) SELECT * FROM time;

-- name_list
WITH t1(a, b) AS (SELECT 1, 2) SELECT * FROM t1;

-- recursive
WITH recursive t2 AS (SELECT 1 AS n UNION ALL SELECT n + 1 FROM t2 WHERE n < 10) SELECT * FROM t2;

-- not materialized
WITH t3 AS NOT MATERIALIZED (SELECT 1) SELECT * FROM t3;

-- materialized
WITH t4 AS MATERIALIZED (SELECT 1) SELECT * FROM t4;

-- multiple cte
WITH t1 AS (SELECT 1), t2 AS (SELECT 2) SELECT * FROM t1 UNION ALL SELECT * FROM t2;

-- comments 1
WITH RECURSIVE /* _SQL_ID_ */ /* block */ -- line
	t AS (SELECT 1)
SELECT 1;

-- comments 2
WITH /* _SQL_ID_ */ t -- with句
AS NOT MATERIALIZED ( --internal_comment
    SELECT * FROM foo -- foo
	-- end
) --test
SELECT * FROM t;

-- comments 3
WITH t1(
	a	-- カラム1
,	b	-- カラム2
) AS (SELECT * FROM t) -- test
,	t2 (
		a	-- カラム1
	,	b	-- カラム2
	,	c	-- カラム3
	,	d	-- カラム4
	) AS MATERIALIZED (SELECT * FROM t) -- test
,	t3 AS NOT MATERIALIZED (
	-- start
	SELECT * FROM t 
	-- end
) -- test
SELECT * FROM t1;
