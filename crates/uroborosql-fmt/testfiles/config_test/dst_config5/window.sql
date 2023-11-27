SELECT
	depname	AS	depname
,	empno		AS	empno
,	salary	AS	salary
,	RANK() OVER(
		PARTITION BY
			depname
		ORDER BY
			salary	DESC
	)
FROM
	empsalary
;
-- 0 argument over
SELECT
	salary							AS	salary	-- salary
,	SUM(salary) OVER()							-- sum
FROM
	empsalary
;
-- frame_clause
SELECT
	order_id	AS	order_id
,	item			AS	item
,	qty				AS	qty
,	SUM(qty) OVER(
		ORDER BY
			order_id
		ROWS	BETWEEN	1	PRECEDING	AND	1	FOLLOWING
	)							result
FROM
	test_orders
;
SELECT
	*
,	STRING_AGG(v, ',') OVER(
		PARTITION BY
			color
		/* partition by */
		ORDER BY
			v
		/* order by */
		GROUPS	BETWEEN	UNBOUNDED	PRECEDING	AND	CURRENT	ROW	EXCLUDE	NO	OTHERS
		/* frame clause with exclusion */
		/* over clause */
	)
FROM
	t
;
