select
	123456789	-- hoge
	as	col
from
	tbl	t
;
select
	1	-- hoge
	as	col1
,	123456789	-- fuga 
	as	col2
from
	tbl	t
;
select
	a	as	a
,	case
		-- case trailing
		/* case */
		when
		-- cond_1
			a	=	1	-- a equals 1
		then
		-- cond_1 == true
			'one'	-- one
		when
		-- cond_2
			a	=	2	-- a equals 2
		then
		-- cond_2 == true
			'two'	-- two
		else
		-- forall i: cond_i == false
			'other'	-- other
	end	-- comment

			as	col
from
	test	-- test table
select
	123456789	-- hoge
	as	col
from
	tbl	t
;
select
	1	-- hoge
	as	col1
,	123456789	-- fuga 
	as	col2
from
	tbl	t
;
select
	a	as	a
,	case
		-- case trailing
		/* case */
		when
		-- cond_1
			a	=	1	-- a equals 1
		then
		-- cond_1 == true
			'one'	-- one
		when
		-- cond_2
			a	=	2	-- a equals 2
		then
		-- cond_2 == true
			'two'	-- two
		else
		-- forall i: cond_i == false
			'other'	-- other
	end	-- comment

			as	col
from
	test	-- test table
where
	case
		when
			a	=	1
		then
			'one'
		else
			'other'
	end
		=
		case
			when
				a	=	1
			then
				'one'
			else
				'other'
		end
;
