select
	id		as	id
,	sum(c)
from
	tbl
group by
	id
having
	sum(c)	>	1
;
select
	id		as	id
,	sum(c)
from
	tbl
group by
	id
having
	sum(c)	>	1
and	avg(c)	<	10
;
select
	id		as	id
,	sum(c)
from
	tbl
group by
	id
having
/* comment */
	sum(c)	>	1
and	avg(c)	<	10
and	max(c)	<=	100
;
