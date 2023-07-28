SELECT
	A	AS	A
,	CASE
		-- case trailing
		/* case */
		WHEN
		-- cond_1
			A	=	1	-- a equals 1
		THEN
		-- cond_1 == true
			'one'	-- one
		WHEN
		-- cond_2
			A	=	2	-- a equals 2
		THEN
		-- cond_2 == true
			'two'	-- two
		ELSE
		-- forall i: cond_i == false
			'other'	-- other
	END
FROM
	TEST	-- test table
