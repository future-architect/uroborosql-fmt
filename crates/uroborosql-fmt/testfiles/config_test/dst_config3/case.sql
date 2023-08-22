select
	ID	as	ID
,	case
		when
			GRADE_POInT	>=	80
		then
			'A'
		when
			GRADE_POInT	<	80
		and	GRADE_POInT	>=	70
		then
			'B'
		when
			GRADE_point	<	70
		and	GRADE_POInT	>=	60
		then
			'C'
		else
			'D'
	end
		as	GRADE
from
	RISYU
where
	SUBJECT_NUMBEr	=	'005'
;
select
	Id	as	Id
,	case
		GRaDE
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
		as	P
from
	RISyU
where
	SUBJECT_NUMber	=	'006'
;
select
	case
		/*param*/A	-- simple case cond
		when
			/*a*/'a'
		then
			'A'
		else
			'B'
	end
