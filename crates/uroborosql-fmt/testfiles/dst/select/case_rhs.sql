SELECT
	A	AS	A
,	(
		SELECT
			Z	AS	Z
		,	CASE
				WHEN
					Z	=	1
				THEN
					'ONE'
				ELSE
					'OTHER'
			END
		FROM
			TAB2
	)
FROM
	TAB1
;
SELECT
	*
FROM
	TBL
WHERE
	TBL.COL	=
		CASE
			WHEN
				COL	IS	NULL
			THEN
				0
			ELSE
				1
		END
AND	CASE
		WHEN
			COL	IS	NULL
		THEN
			0
		ELSE
			1
	END
			>	FUNC(HOGE)