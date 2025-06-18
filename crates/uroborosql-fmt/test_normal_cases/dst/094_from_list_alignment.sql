select
	*
from
	table1	t1				-- regular table with alias
,	(
		select
			id		as	id
		,	name	as	name
		from
			users
		where
			active	=	true
	)		u				-- subquery with alias
,	table2	t2
left outer join
	table3	t3
on
	t2.id	=	t3.id	-- joined table condition 1
and	t2.name	=	t3.name	-- joined table condition 2
,	(
		table4	t4
	inner join
		table5	t5
	on
		t4.ref_id	=	t5.id
	)		joined_tables	-- parenthesized join with alias
where
	t1.user_id	=	u.id
;
