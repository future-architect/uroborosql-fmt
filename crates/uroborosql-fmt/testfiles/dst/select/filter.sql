select
	city			as	city
,	count(*) filter(
		where
			temp_lo	<	45
	)
,	max(temp_lo)
from
	weather
group by
	city
;
