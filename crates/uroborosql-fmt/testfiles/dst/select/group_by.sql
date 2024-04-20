select
	id			as	id
,	sum(cnt)
from
	tbl
group by
	id
having
	sum(cnt)	>	1
and	avg(cnt)	<	10
;
