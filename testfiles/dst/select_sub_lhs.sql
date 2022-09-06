SELECT
	A
,	(
		SELECT
			Z
		FROM
			TAB2
	)
FROM
	LONGLONGTABLE	L
,	(
		SELECT
			B
		,	C
		FROM
			TAB1
	)				BC
