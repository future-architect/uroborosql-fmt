select
	123456789	-- hoge
	as	COL
from
	TBL	T
;
select
	1	-- hoge
	as	COL1
,	123456789	-- fuga 
	as	COL2
from
	TBL	T
;
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
	END	-- comment

			as	COL
FROM
	TEST	-- test table
select
	123456789	-- hoge
	COL
from
	TBL	T
;
select
	1	-- hoge
	COL1
,	123456789	-- fuga 
	COL2
from
	TBL	T
;
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
	END	-- comment

				COL
FROM
	TEST	-- test table
where
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
