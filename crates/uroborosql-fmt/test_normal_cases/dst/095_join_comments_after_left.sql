select
	*
from
	t1	-- t1 trailing
-- t1
inner join
	t2
on
	t1.num	=	t2.num
;
select
	a	as	a
from
	(
		select
			1
	)	t1	-- t1 trailing
-- t1
inner join
	(
		select
			1
	)	t2	-- t2 trailing
on
	1	=	1
;
