select
	a	as	a
from
	longlongtable		l	-- so long
,	tab						-- no alias
,	table1				t1	-- normal
,	sososolonglonglong		-- so long and no alias
where
	l.a						=	l.b							-- normal
and	sososolonglonglong.a	=	1							-- so long 
or	t1.x	+	t1.y		=	42							-- long lhs
and	tab.a					=	1	+	2	+	3	+	5	-- long rhs
