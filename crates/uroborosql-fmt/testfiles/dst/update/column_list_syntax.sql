update
	weather
set
	(temp_lo, temp_hi, prcp)	=	(temp_lo	+	1, temp_lo	+	15, default)
where
	city	=	'San Francisco'
and	date	=	'2003-07-03'
;
update
	accounts
set
	(contact_first_name, contact_last_name)	=	(
		select
			first_name	as	first_name
		,	last_name	as	last_name
		from
			salesmen
		where
			salesmen.id	=	accounts.sales_id
	)
;
