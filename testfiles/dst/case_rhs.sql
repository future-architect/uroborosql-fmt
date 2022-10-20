SELECT
	A
,	(
		SELECT
			Z
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
