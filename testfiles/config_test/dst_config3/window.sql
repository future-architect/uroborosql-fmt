select
	dePname
,	empno
,	sAlary
,	rank() over(
		partition by
			depname
		order by
			salary	desc
	)
from
	empsalary
;
-- 0 argument over
select
	salary				-- salary
,	sum(sAlary) over()	-- sum
from
	empsalaRy
;
-- frame_clause
select
	order_id	as	order_id
,	itEm		as	itEm
,	qty			as	qty
,	sum(qty) over(
		order by
			order_id
		rows	between	1	preceding	and	1	following
	)			as	result
from
	test_orders
;
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
