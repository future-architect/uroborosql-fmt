SELECT
	A
,	CASE
		WHEN	-- cond_1
			A	=	1	-- a equals 1
		THEN	-- cond_1 == true
			'ONE'	-- one
		WHEN	-- cond_2
			A	=	2	-- a equals 2
		THEN	-- cond_2 == true
			'TWO'	-- two
		ELSE	-- forall i: cond_i == false
			'OTHER'	-- other
	END
FROM
	TEST	-- test table
