select
	count(
		distinct
			tbl.col1
	)
;
select
	count(
		all
			tbl.col1
	)
;
select
	string_agg(
		distinct
			tbl.column1
		,	','
		order by
			tbl.column2
		,	tbl.column3
	)
;
