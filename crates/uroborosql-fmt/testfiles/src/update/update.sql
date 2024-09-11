UPDATE TABLE1 TBL1 -- テーブル1
SET TBL1.COLUMN2 = 100 -- カラム2
, TBL1.COLUMN3 = 100 -- カラム3
WHERE TBL1.COLUMN1	=	10
;

update
	T	t
set -- after set keyword
-- another comment
	c	=	c	+	1
where
	id	=	1
;
