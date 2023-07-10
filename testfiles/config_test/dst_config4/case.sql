SELECT
	ID	As	ID
,	case
		when
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
	EnD
		As	GRADE
FROM
	RISYU
WHERe
	SUBJECT_NUMBER	=	'005'
;
SELECt
	ID	AS	ID
,	CAse
		GRADE
		WHeN
			'A'
		ThEN
			5
		WHEn
			'B'
		THen
			4
		WHen
			'C'
		Then
			3
		ELSE
			0
	End
		AS	P
FROm
	RISYU
WHere
	SUBJECT_NUMBER	=	'006'
;
SELECt
	cASe
		/*param*/A	-- simple case cond
		WHeN
			/*a*/'a'
		THEn
			'A'
		Else
			'B'
	eND
