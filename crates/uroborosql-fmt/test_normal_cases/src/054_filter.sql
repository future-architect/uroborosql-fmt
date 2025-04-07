-- simple filter
SELECT
	count(*) FILTER (WHERE a > 10)
FROM
	t
;

-- multiple filters with alias
SELECT
	count(*) FILTER (WHERE a > 10) as high_count,
	count(*) FILTER (WHERE a <= 10) as low_count
FROM
	t
;

-- filter with comment
SELECT
	count(*) FILTER (WHERE -- comment
		a > 10
	-- comment
	)
FROM
	t
;
