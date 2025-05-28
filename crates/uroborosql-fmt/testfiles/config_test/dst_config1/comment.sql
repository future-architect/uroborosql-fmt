SELECT /* _SQL_ID_ */
	123456789	-- hoge
	AS	COL
FROM
	TBL	T
;
SELECT
	1	-- hoge
	AS	COL1
,	123456789	-- fuga 
	AS	COL2
FROM
	TBL	T
;
SELECT
	A
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
	END	-- comment

			AS	COL
FROM
	TEST	-- test table
;
SELECT
	123456789	-- hoge
	AS	COL
FROM
	TBL	T
;
SELECT
	1	-- hoge
	AS	COL1
,	123456789	-- fuga 
	AS	COL2
FROM
	TBL	T
;
SELECT
	A
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
	END	-- comment

			AS	COL
FROM
	TEST	-- test table
WHERE
	CASE
		WHEN
			A	=	1
		THEN
			'one'
		ELSE
			'other'
	END
		=
		CASE
			WHEN
				A	=	1
			THEN
				'one'
			ELSE
				'other'
		END
;
