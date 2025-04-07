-- just over
SELECT a, avg(a) over()
FROM t
;

-- partition by
SELECT a, avg(a) OVER (PARTITION BY b)
FROM t
;

-- partition by and order by
SELECT a, avg(a) OVER (PARTITION BY b ORDER BY c)
FROM t
;

-- partition by and order by and frame clause
SELECT a, avg(a) OVER (
    PARTITION BY b
    ORDER BY c
    ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING
)
FROM t
;

-- partition by and order by and frame clause and exclusion
SELECT a, avg(a) OVER (
    partition by b
    order by c
    groups between unbounded preceding and current row 
    exclude no others
)
FROM t
;
-- comments
SELECT
	*
,	string_agg(v, ',') OVER(
		PARTITION BY color
		/* partition by */
		ORDER BY v
		/* order by */
		GROUPS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE NO OTHERS
		/* frame clause with exclusion */
		/* over clause */
	)
FROM t
;
