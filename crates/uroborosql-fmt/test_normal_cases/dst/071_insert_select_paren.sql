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
