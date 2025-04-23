with
	t	as	(
		select
			*
		from
			distributors
		where
			active	=	true
	)
delete
from
	distributors
using
	t
where
	distributors.id	=	t.id
;
with
	t1	as	not materialized	(
		select
			*
		from
			tbl1
		where
			value	>	0
	)
,	t2	as	(
		select
			*
		from
			tbl2
		where
			flag	=	true
	)
-- comment
-- comment
delete
from
	tbl1
using
	t1
,	t2
where
	tbl1.id	=	t1.id
and	t2.flag	=	true
;
