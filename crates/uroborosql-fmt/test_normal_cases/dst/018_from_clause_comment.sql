select
	a	as	a
from
	t	-- comment
;
select
	a	as	a
from
	t.a	-- comment
;
select
	u.*
from
	test_schema.users	-- comment
;
select
	a	as	a
from
	longlongtable		l	-- so long
,	tab						-- no alias
,	table1				t1	-- normal
,	sososolonglonglong		-- so long and no alias
;
