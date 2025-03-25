select
	*
from
	department
where
	id		in	(
		select
			department_id	as	department_id
		from
			users
		where
			address	=	'TOKYO'
	)
and	test	=	test
