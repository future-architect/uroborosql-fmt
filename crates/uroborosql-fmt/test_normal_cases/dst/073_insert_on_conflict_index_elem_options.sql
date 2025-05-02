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
;
