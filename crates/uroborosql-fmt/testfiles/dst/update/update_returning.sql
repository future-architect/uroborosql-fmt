UPDATE
	PRODUCTS
SET
	PRICE	=	PRICE	*	1.10
WHERE
	PRICE	<=	99.99
RETURNING
	NAME
,	PRICE	AS	NEW_PRICE
;