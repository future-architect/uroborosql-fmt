update /* _SQL_ID_ */
	tbl	t
set
	t.col2	=	a.col2
,	t.col3	=	a.col3
from
	tbl_a	a

left outer join
	tbl_b	b
on
	a.col1	=	b.col1
where
	t.id	=	a.id
