-- simple table_function
select
	*
from
	unnest(a)	t
;
select
	*
from
	unnest(a)	t
;
select
	*
from
	unnest(a) with ordinality	t
;
select
	*
from
	unnest(a) with ordinality	t
;
-- alias with `name_list`
select
	*
from
	unnest(a) with ordinality	t(i, v)
;
select
	*
from
	unnest(a) with ordinality	t(i, v)
;
-- alias with `TableFuncElementList`
select
	*
from
	unnest(a) with ordinality	t(i	int, v	text)
;
select
	*
from
	unnest(a) with ordinality	t(i	int, v	text)
;
-- `TableFuncElementList` かつ、 as が省略不可のケース（省略すると構文上不正になりパースできない）
select
	*
from
	unnest(a) with ordinality	as	(id	int, v	text)	-- as は省略されない
;
-- alignment
select
	*
from
	test						t1
,	unnest(a) with ordinality	t2
,	unnest(b) with ordinality	t3(i	int, v	text)
,	unnest(c) with ordinality	t4(
		i	int		-- comment
	,	v	text
	)
;
-- comment
select
	*
from
	unnest(a) with ordinality	t_a(
		i	-- comment 1
	,	v	-- comment 2
	)
,	unnest(b) with ordinality	t_b(
		i	-- comment 1
	,	v	-- comment 2
	)
,	unnest(c) with ordinality	t_c(
		i	int		-- comment 1
	,	v	text	-- comment 2
	)
,	unnest(d) with ordinality	t_d(
		i	int		-- comment 1
	,	v	text	-- comment 2
	)
;
