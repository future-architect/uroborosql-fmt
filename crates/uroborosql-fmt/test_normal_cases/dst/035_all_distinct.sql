-- all keyword
select
	all
	itemid		as	itemid
,	itemname	as	itemname
;
-- distinct keyword
select
	distinct
	itemid		as	itemid
,	itemname	as	itemname
;
-- distinct clause
select
	distinct on
		(
			quantity
		,	itemname
		,	area
		)
	itemid		as	itemid
,	itemname	as	itemname
;
