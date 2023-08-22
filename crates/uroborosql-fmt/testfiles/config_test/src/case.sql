SELECT
	ID	As	ID
,	case
		when
			GRADE_POInT	>=	80
		THEN
			'A'
		WHEN
			GRADE_POInT	<	80
		AND	GRADE_POInT	>=	70
		THEN
			'B'
		WHEN
			GRADE_point<	70
		AND	GRADE_POInT	>=	60
		THEN
			'C'
		ELSE
			'D'
	EnD	As	GRADE
FROM
	RISYU
WHERe
	SUBJECT_NUMBEr	=	'005'
;
SELECt
	Id	
,	CAse
		GRaDE
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
	End	AS	P
FROm
	RISyU
WHere
	SUBJECT_NUMber	=	'006'
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
