-- simple update
update
	t
set
	t.t1	=	100
,	t.t2	=	200
where
	t.t3	=	300
;
-- comments
update
	table1	tbl1	-- テーブル1
set
	tbl1.column2	=	100	-- カラム2
,	tbl1.column3	=	100	-- カラム3
where
	tbl1.column1	=	10
;
update
	t	t
set
-- after set keyword
-- another comment
	c	=	c	+	1
where
	id	=	1
;
