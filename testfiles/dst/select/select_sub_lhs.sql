SELECT
	A
,	(
		-- comm1
		/* comm2 */
		SELECT
			Z
		/* z */
		FROM
			TAB2
		/* comm3*/
	)
FROM
	LONGLONGTABLE	AS	L
,	(
		SELECT
			B
		,	C
		FROM
			TAB1
	)	-- trailing
					AS	BC
