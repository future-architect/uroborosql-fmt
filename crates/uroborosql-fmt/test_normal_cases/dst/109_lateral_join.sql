-- LATERAL JOIN with subquery
select
	c.category_name	as	category_name
,	p.product_name	as	product_name
,	p.created_at	as	created_at
from
	categories	c
left outer join
	lateral(
		select
			pr.product_name	as	product_name
		,	pr.created_at	as	created_at
		from
			products	pr
		where
			pr.category_id	=	c.id
		order by
			pr.created_at	desc
		limit	3
	)	p
on
	true
;
-- LATERAL with INNER JOIN
select
	t1.id		as	id
,	t2.value	as	value
from
	t1
inner join
	lateral(
		select
			*
		from
			t2
		where
			t2.t1_id	=	t1.id
	)	t2
on
	true
;
-- LATERAL with CROSS JOIN
select
	t1.id		as	id
,	t2.value	as	value
from
	t1
cross join
	lateral(
		select
			*
		from
			t2
		where
			t2.t1_id	=	t1.id
	)	t2
;
-- LATERAL without AS keyword
select
	c.name		as	name
,	p.product	as	product
from
	categories	c
left outer join
	lateral(
		select
			product	as	product
		from
			products
		where
			category_id	=	c.id
		limit	1
	)	p
on
	true
;
-- Multiple LATERAL JOINs
select
	a.id	as	id
,	b.val	as	val
,	c.val	as	val
from
	table_a	a
left outer join
	lateral(
		select
			val	as	val
		from
			table_b
		where
			a_id	=	a.id
		limit	1
	)	b
on
	true
left outer join
	lateral(
		select
			val	as	val
		from
			table_c
		where
			a_id	=	a.id
		limit	1
	)	c
on
	true
;
