select
	*
from
	tbl	t
where
	t.id	=	(
		select
			max(t2.id)
		from
			tbl	t2
	)
and	t.age	<	100
;
select
	*
from
	tbl	t
where
	t.id	=	(
		select
			max(t2.id)
		from
			tbl	t2
	)
or	t.id	=	2
;
select
	*
from
	tbl	t
where -- comment
	t.id	=	(
		select
			max(t2.id)
		from
			tbl	t2
	)
and	-- comment
	-- comment
	t.age	<	100
;
select
	*
from
	tbl	t
where -- comment
	t.id	=	(
		select
			max(t2.id)
		from
			tbl	t2
	)
or	-- comment
	-- comment
	t.id	=	2
;
