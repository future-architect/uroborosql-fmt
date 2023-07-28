SELECT
	A	AS	A
,	(
		-- comm1
		/* comm2 */
		SELECT
			Z	AS	Z
		/* z */
		FROM
			TAB2
		/* comm3*/
	)
FROM
	LONGLONGTABLE	L
,	(
		SELECT
			B	AS	B
		,	C	AS	C
		FROM
			TAB1
	)	-- trailing
					BC
