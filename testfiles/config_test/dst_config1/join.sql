SELECT /* _SQL_ID_ */
	*
FROM
	T1
INNER JOIN
	T2
ON
	T1.NUM	=	T2.NUM
;
SELECT
	*
FROM
	T1
LEFT JOIN
	T2
ON
	T1.NUM	=	T2.NUM
;
SELECT
	*
FROM
	T1
RIGHT JOIN
	T2
ON
	T1.NUM	=	T2.NUM
;
SELECT
	*
FROM
	T1
FULL JOIN
	T2
ON
	T1.NUM	=	T2.NUM
;
