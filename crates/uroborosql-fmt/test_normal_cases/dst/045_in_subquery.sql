select
	col1	in	(
		select
			col1	as	col1
		from
			table1
		where
			col2	=	'col2'
	)
;
select
	*
from
	table1
where
	col1	in	(
		select
			col1	as	col1
		from
			table1
		where
			col2	=	'col2'
	)
and	test	=	test
;
