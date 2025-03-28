-- 単純CASE式
select
	case
		grade
		when
			'A'
		then
			5
		when
			'B'
		then
			4
		when
			'C'
		then
			3
		else
			0
	end
		as	p
;
-- 検索CASE式
select
	case
		when
			grade_point	>=	80
		then
			'A'
		when
			grade_point	<	80
		and	grade_point	>=	70
		then
			'B'
		when
			grade_point	<	70
		and	grade_point	>=	60
		then
			'C'
		else
			'D'
	end
		as	p
;
-- without else
select
	case
		when
			grade_point	>=	80
		then
			'A'
		when
			grade_point	<	80
		and	grade_point	>=	70
		then
			'B'
		when
			grade_point	<	70
		and	grade_point	>=	60
		then
			'C'
	end
		as	p
;
-- bind param
select
	case
		/*param*/a	-- simple case cond
		when
			/*a*/'a'
		then
			'A'
		else
			'B'
	end
;
-- comments
select
	case
		a	-- comment 1
		-- comment 2
		when
		-- after when
			'a'	-- after expr
		then
		-- after then
			'A'	-- after expr
		else
		-- after else
			'B'	-- after expr
		-- before end
	end
;
