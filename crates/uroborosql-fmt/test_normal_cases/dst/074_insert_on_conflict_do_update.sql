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
