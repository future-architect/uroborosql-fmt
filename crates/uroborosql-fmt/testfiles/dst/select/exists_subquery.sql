select
	*
from
	department
where
	exists(
		select
			department_id	as	department_id
		from
			users
		where
			address	=	'TOKYO'
	)
and	test	=	test
