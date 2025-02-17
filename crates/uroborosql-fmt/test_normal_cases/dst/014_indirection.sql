-- Indirection (table qualifier)
select
	tbl0.*
,	tbl1.a		as	a
,	tbl1.b		as	b
,	tbl2.a.c	as	c
,	tbl2.a.b.d	as	d
,	tbl2.a.b.c	as	original_c
,	tbl2.a.b.e	as	e
,	tbl2.a.b.f	as	f
;
