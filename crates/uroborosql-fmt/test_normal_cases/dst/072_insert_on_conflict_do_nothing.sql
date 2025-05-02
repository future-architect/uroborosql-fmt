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
	conflict
on
	constraint	DISTRIBUTORS_PKEY
do
	nothing
;
