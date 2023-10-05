insert
into
	distributors	as	d
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
