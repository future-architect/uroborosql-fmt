-- simple update
UPDATE t
SET t.t1 = 100
, t.t2 = 200
WHERE t.t3 = 300
;

-- comments
UPDATE TABLE1 TBL1 -- テーブル1
SET TBL1.COLUMN2 = 100 -- カラム2
, TBL1.COLUMN3 = 100 -- カラム3
WHERE TBL1.COLUMN1	= 10
;

UPDATE T	t
SET -- after set keyword
-- another comment
	c	= c	+ 1
WHERE
	id	= 1
;
