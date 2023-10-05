select
	id			as	id
,	avg(grade)
from
	student
group by
	id
;
select
	concat_lower_or_upper(
		'Hello'	-- hello
	,	'World'	-- world
	,	true	-- true
	)
;
select
	func(
		case
			when
				flag
			then
				a
			else
				b
		end
	,	c
	)
;
select
	city			as	city
,	max(temp_lo)
from
	weather
group by
	city
having
	max(temp_lo)	<	40
;
select
	func((a	-	b), c)
