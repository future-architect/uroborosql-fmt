select
	*
from
	tbl t
where
	(
		t.a && /*a*/array[0]
		and t.b && array[1, 2]
		and t.c && array[]::integer[]
	)
	or (
		t.d && array[10, 20, 30]
		and t.e && /*b*/array[col1, col2]
	)
	or t.f && array['x', 'y']
