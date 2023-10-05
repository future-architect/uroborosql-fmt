select
	*
from
	department
where
	id		in	(
		select
			department_id	as	department_id
		from
			user
		where
			address	=	'TOKYO'
	)
and	test	=	test
