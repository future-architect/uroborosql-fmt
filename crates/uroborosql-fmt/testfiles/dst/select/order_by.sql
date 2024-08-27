select
	col	as	col
from
	tab
order by
	col			asc					-- 昇順
,	long_col	desc nulls first	-- 降順
,	null_col	nulls first			-- NULL先
select
	*
from
	foo	t
order by
	t.bar1
,	/* after comma */
	t.bar2
,	t.bar3
select
	*
from
	foo	t
order by
	t.bar1
/* before comma */
,	t.bar2
,	t.bar3
