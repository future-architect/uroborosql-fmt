-- simple with
with
	t	as	(
		select
			1
	)
select
	*
from
	t
;
-- special name
with
	time	as	(
		select
			1
	)
select
	*
from
	t
;
-- name_list
with
	t1	(
		a
	,	b
	)	as	(
		select
			1
		,	2
	)
select
	*
from
	t1
;
-- recursive
with recursive
	t2	as	(
		select
			1	as	n
		union all
		select
			n	+	1
		from
			t2
		where
			n	<	10
	)
select
	*
from
	t2
;
-- not materialized
with
	t3	as	not materialized	(
		select
			1
	)
select
	*
from
	t3
;
-- materialized
with
	t4	as	materialized	(
		select
			1
	)
select
	*
from
	t4
;
-- multiple cte
with
	t1	as	(
		select
			1
	)
,	t2	as	(
		select
			2
	)
select
	*
from
	t1
union all
select
	*
from
	t2
;
-- comments 1
with recursive /* _SQL_ID_ */
/* block */
-- line
	t	as	(
		select
			1
	)
select
	1
;
-- comments 2
with /* _SQL_ID_ */
	t	-- with句
	as	not materialized	(
		--internal_comment
		select
			*
		from
			foo	-- foo
		-- end
	)	-- test
select
	*
from
	t
;
-- comments 3
with
	t1	(
		a	-- カラム1
	,	b	-- カラム2
	)	as	(
		select
			*
		from
			t
	)	-- test
,	t2	(
		a	-- カラム1
	,	b	-- カラム2
	,	c	-- カラム3
	,	d	-- カラム4
	)	as	materialized	(
		select
			*
		from
			t
	)	-- test
,	t3	as	not materialized	(
		-- start
		select
			*
		from
			t
		-- end
	)	-- test
select
	*
from
	t1
;
