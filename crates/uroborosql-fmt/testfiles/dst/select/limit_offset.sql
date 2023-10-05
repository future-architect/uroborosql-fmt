select
	tbl1.column1	as	column1
from
	table1	tbl1
order by
	tbl1.column2	desc
limit	5
offset	5
;
select
	tbl1.column1	as	column1
from
	table1	tbl1
order by
	tbl1.column2	desc
limit	/*$hoge*/5
offset	5
;
select
	tbl1.column1	as	column1
from
	table1	tbl1
order by
	tbl1.column2	desc
limit	all
offset	5
;
select
	tbl1.column1	as	column1
from
	table1	tbl1
order by
	tbl1.column2	desc
limit	/*$hoge*/all
offset	5
;
