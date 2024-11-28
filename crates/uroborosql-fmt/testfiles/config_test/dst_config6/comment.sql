SELECT
	123456789	-- hoge
	AS	col
FROM
	tbl	t
;
SELECT
	1	-- hoge
	AS	col1
,	123456789	-- fuga 
	AS	col2
FROM
	tbl	t
;
SELECT
	a
,	CASE
		-- case trailing
		/* case */
		WHEN
		-- cond_1
			a	=	1	-- a equals 1
		THEN
		-- cond_1 == true
			'one'	-- one
		WHEN
		-- cond_2
			a	=	2	-- a equals 2
		THEN
		-- cond_2 == true
			'two'	-- two
		ELSE
		-- forall i: cond_i == false
			'other'	-- other
	END	-- comment

			AS	col
FROM
	test	-- test table
SELECT
	123456789	-- hoge
	col
FROM
	tbl	t
;
SELECT
	1	-- hoge
	col1
,	123456789	-- fuga 
	col2
FROM
	tbl	t
;
SELECT
	a
,	CASE
		-- case trailing
		/* case */
		WHEN
		-- cond_1
			a	=	1	-- a equals 1
		THEN
		-- cond_1 == true
			'one'	-- one
		WHEN
		-- cond_2
			a	=	2	-- a equals 2
		THEN
		-- cond_2 == true
			'two'	-- two
		ELSE
		-- forall i: cond_i == false
			'other'	-- other
	END	-- comment

			col
FROM
	test	-- test table
WHERE
	CASE
		WHEN
			a	=	1
		THEN
			'one'
		ELSE
			'other'
	END
		=
		CASE
			WHEN
				a	=	1
			THEN
				'one'
			ELSE
				'other'
		END
;
