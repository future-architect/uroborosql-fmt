select
	array[1, 2, 3]								as	basic_array
,	array[]										as	empty_array
,	cast(array[]	as	integer[])				as	typed_empty_array
,	cast(array[1, 2]	as	integer[])			as	typed_array
,	coalesce(col, cast(array[]	as	integer[]))	as	coalesced_array
,	array['a', 'b', 'c']						as	string_array
,	array[col1, col2, col3]						as	column_array
;
select
	array[[1, 2], [3, 4]]
;
