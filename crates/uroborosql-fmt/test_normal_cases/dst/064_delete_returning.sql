delete
from
	products
where
	obsoletion_date	=	'today'
returning
	*
;
