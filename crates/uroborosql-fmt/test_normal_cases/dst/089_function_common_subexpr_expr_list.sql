-- COALESCE function tests
select
	coalesce(null, null, 'default')
;
select
	coalesce(column_a, column_b, 'fallback')
from
	table_name
;
select
	coalesce(1, 2, 3, 4, 5)
;
-- GREATEST function tests  
select
	greatest(10, 5, 20, 15)
;
select
	greatest('apple', 'banana', 'orange')
;
select
	greatest(
		cast('2023-01-15'	as	date)
	,	cast('2023-03-10'	as	date)
	,	cast('2022-12-25'	as	date)
	)
;
select
	greatest(10, null, 20, 5)
;
-- LEAST function tests
select
	least(10, 5, 20, 15)
;
select
	least('apple', 'banana', 'orange')
;
select
	least(
		cast('2023-01-15'	as	date)
	,	cast('2023-03-10'	as	date)
	,	cast('2022-12-25'	as	date)
	)
;
select
	least(10, null, 20, 5)
;
-- XMLCONCAT function tests
select
	xmlconcat(
		'<tag1>Value 1</tag1>'
	,	'<tag2>Value 2</tag2>'
	)
;
select
	xmlconcat(
		'<a>Text A</a>'
	,	'<b>Text B</b>'
	,	'<c>Text C</c>'
	)
;
select
	xmlconcat(xml_column1, xml_column2)
from
	xml_table
;
-- Complex query with multiple expr_list functions
select
	product_name						as	product_name
,	coalesce(price_a, price_b, 0)		as	coalesced_price
,	greatest(price_a, price_b, price_c)	as	highest_price
,	least(price_a, price_b, price_c)	as	lowest_price
,	xmlconcat(
		'<product>'
	,	product_name
	,	'</product>'
	)									as	xml_product
from
	product_prices
;
