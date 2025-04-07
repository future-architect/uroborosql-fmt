-- simple filter
select
	count(*) filter(
		where
			a	>	10
	)
from
	t
;
-- multiple filters with alias
select
	count(*) filter(
		where
			a	>	10
	)	as	high_count
,	count(*) filter(
		where
			a	<=	10
	)	as	low_count
from
	t
;
-- filter with comment
select
	count(*) filter(
		where
		-- comment
			a	>	10
		-- comment
	)
from
	t
;
