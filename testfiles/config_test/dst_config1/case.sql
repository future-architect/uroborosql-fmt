SELECT
	ID	AS	ID
,	CASE
		WHEN
			GRADE_POINT	>=	80
		THEN
			'A'
		WHEN
			GRADE_POINT	<	80
		AND	GRADE_POINT	>=	70
		THEN
			'B'
		WHEN
			GRADE_POINT	<	70
		AND	GRADE_POINT	>=	60
		THEN
			'C'
		ELSE
			'D'
	END
		AS	GRADE
FROM
	RISYU
WHERE
	SUBJECT_NUMBER	=	'005'
;
SELECT
	ID
,	CASE
		GRADE
		WHEN
			'A'
		THEN
			5
		WHEN
			'B'
		THEN
			4
		WHEN
			'C'
		THEN
			3
		ELSE
			0
	END
		AS	P
FROM
	RISYU
WHERE
	SUBJECT_NUMBER	=	'006'
;
SELECT
	CASE
		/*param*/A	-- simple case cond
		WHEN
			/*a*/'a'
		THEN
			'A'
		ELSE
			'B'
	END
