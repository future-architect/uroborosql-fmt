select
	*
from
	tbl1
where
	exists(
		select
			col1	as	col1
		from
			tbl2
		where
			col1	=	'foo'
	)
;
select
	*
from
	tbl1
where
	exists(
		select
			col1	as	col1
		from
			tbl2
		where
			col1	=	'foo'
	)
and	test	=	test
;
