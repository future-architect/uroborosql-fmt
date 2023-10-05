insert
into
	distributors
(
	did
,	dname
) values (
	default
,	'XYZ Widgets'
)
returning
	did
;
