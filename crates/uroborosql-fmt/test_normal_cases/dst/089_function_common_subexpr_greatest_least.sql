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
select
	greatest(null, null, null)
;
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
select
	least(null, null, null)
;
select
	product_name						as	product_name
,	greatest(price_a, price_b, price_c)	as	highest_price
,	least(price_a, price_b, price_c)	as	lowest_price
from
	product_prices
;
