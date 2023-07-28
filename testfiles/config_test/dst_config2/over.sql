SELECT
	depname	AS	depname
,	empno	AS	empno
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
