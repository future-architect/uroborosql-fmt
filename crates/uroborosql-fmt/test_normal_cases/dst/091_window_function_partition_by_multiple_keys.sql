select
	id			as	id
,	category	as	category
,	subcategory	as	subcategory
,	region		as	region
,	value		as	value
,	row_number() over(
		partition by
			category
		,	subcategory
		,	region
		order by
			value
	)			as	rn
from
	test_table
;
