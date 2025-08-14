insert
into
	stafflist
(
	name
,	address
,	staff_cls
)
(
	select
		name							as	name
	,	address							as	address
	,	/*#CLS_STAFF_CLS_NEW_COMER*/'0'
	from
		newcomer
	where
		flag	=	'TRUE'
);
insert
into
	t
(
	id
)
-- comment
-- comment2
select
	id	as	id
from
	t2
where
	id	=	1
;
