select
	substring('Hello World', 1, 5)		as	basic_substring
,	substring(column_name, 2, 10)		as	column_substring
,	substring('test string', 6)			as	substring_from_position
,	substring(/*bind_param*/'test', 6)	as	bind_param_substring
,	substring(
		concat(first_name, ' ', last_name)
	,	1
	,	20
	)									as	complex_substring
,	substring(
		description
	,	length(description)	-	10
	)									as	end_substring
from
	users
where
	substring(email, 1, 5)	=	'admin'
;
