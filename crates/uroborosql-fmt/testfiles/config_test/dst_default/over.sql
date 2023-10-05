select
	depname	as	depname
,	empno	as	empno
,	salary	as	salary
,	rank() over(
		partition by
			depname
		order by
			salary	desc
	)
from
	empsalary
;
