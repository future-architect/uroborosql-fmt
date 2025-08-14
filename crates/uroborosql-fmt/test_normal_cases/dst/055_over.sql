-- just over
select
	a				as	a
,	avg(a) over()
from
	t
;
-- partition by
select
	a	as	a
,	avg(a) over(
		partition by
			b
	)
from
	t
;
-- partition by and order by
select
	a	as	a
,	avg(a) over(
		partition by
			b
		order by
			c
	)
from
	t
;
-- partition by and order by and frame clause
select
	a	as	a
,	avg(a) over(
		partition by
			b
		order by
			c
		rows	between	1	preceding	and	1	following
	)
from
	t
;
-- partition by and order by and frame clause and exclusion
select
	a	as	a
,	avg(a) over(
		partition by
			b
		order by
			c
		groups	between	unbounded	preceding	and	current	row	exclude	no	others
	)
from
	t
;
-- comments
select
	*
,	string_agg(v, ',') over(
		partition by
			color
		/* partition by */
		order by
			v
		/* order by */
		groups	between	unbounded	preceding	and	current	row	exclude	no	others
		/* frame clause with exclusion */
		/* over clause */
	)
from
	t
;
