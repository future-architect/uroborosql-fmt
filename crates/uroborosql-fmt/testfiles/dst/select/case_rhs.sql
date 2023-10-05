select
	a	as	a
,	(
		select
			z	as	z
		,	case
				when
					z	=	1
				then
					'ONE'
				else
					'OTHER'
			end
		from
			tab2
	)
from
	tab1
;
select
	*
from
	tbl
where
	tbl.col	=
		case
			when
				col	is	null
			then
				0
			else
				1
		end
and	case
		when
			col	is	null
		then
			0
		else
			1
	end
			>	func(hoge)
