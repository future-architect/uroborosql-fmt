select
	DEpNAME	as	DEpNAME
,	EMPNo	as	EMPNo
,	SALARY	as	SALARY
,	rank() over(
		partition by
			DEPNAME
		order by
			SALARY	desc
	)
from
	EMPSALARY
;
