-- do nothing
insert
into
	distributors
(
	did
,	dname
) values (
	9
,	'Antwerp Design'
)
on
	conflict	(
		did1
	,	did2
	,	did3
	)
do
	nothing
;
-- on constraint name
insert
into
	distributors
(
	did
,	dname
) values (
	9
,	'Antwerp Design'
)
on
	conflict
on
	constraint	DISTRIBUTORS_PKEY
do
	nothing
;
-- collate
insert
into
	distributors
(
	did
,	dname
) values (
	9
,	'Antwerp Design'
)
on
	conflict	(
		did1	collate	"x"	int4_ops
	,	did2	collate	"x"	int4_ops
	)
do
	nothing
;
-- do update set where
insert
into
	distributors
(
	did
,	dname
) values (
	8
,	'Anvil Distribution'
)
on
	conflict	(
		did
	)
do
	update
	set
		dname	=	excluded.dname	||	' (formerly '	||	d.dname	||	')'
	where
		d.zipcode	!=	'21201'
;
-- comment after query
insert
into
	distributors
(
	did
,	dname
)
select
	id	as	id
from
	tbl2
where
	id	=	1	-- trailing comment
on
	conflict
do
	nothing
;
