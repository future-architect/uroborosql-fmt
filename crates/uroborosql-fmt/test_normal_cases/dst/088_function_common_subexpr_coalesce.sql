select
	coalesce(
		null
	,	null
	,	'apple'
	,	'banana'
	,	null
	,	'orange'
	)
;
select
	name							as	name
,	coalesce(description, 'N/A')	as	product_description
,	coalesce(price, 0.00)			as	product_price
from
	products
;
