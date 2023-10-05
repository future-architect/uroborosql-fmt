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
	end
from
	test	-- test table
