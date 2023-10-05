update
	products
set
	price	=	price	*	1.10
where
	price	<=	99.99
returning
	name
,	price	as	new_price
;
