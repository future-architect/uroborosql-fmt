update
	table1	tbl1	-- テーブル1
set
	tbl1.column2	=	tbl2.columnx	-- カラム2
,	tbl1.column3	=	100				-- カラム3
-- コメント
from
	table2	tbl2	-- テーブル2
where
	tbl1.column1	=	10
and	tbl1.column4	=	tbl2.columny
