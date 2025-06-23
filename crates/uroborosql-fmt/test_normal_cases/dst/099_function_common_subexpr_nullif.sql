select
	nullif(column1, '')
,	nullif(price, 0)
,	nullif(
		case
			when
				status	=	'active'
			then
				'A'
			else
				'I'
		end
	,	'I'
	)
,	nullif(id, parent_id)
;
