SELECT
	*
FROM
	t1
INNER JOIN
	t2
ON
	t1.num	=	t2.num
;
SELECT
	*
FROM
	t1
LEFT OUTER JOIN
	t2
ON
	t1.num	=	t2.num
;
SELECT
	*
FROM
	t1
RIGHT OUTER JOIN
	t2
ON
	t1.num	=	t2.num
;
SELECT
	*
FROM
	t1
FULL OUTER JOIN
	t2
ON
	t1.num	=	t2.num
;
