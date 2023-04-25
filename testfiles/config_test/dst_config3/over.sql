select
	DEpNAME
,	EMPNo
,	SALARY
,	rank() over(
		partition by
			DEPNAME
		order by
			SALARY	desc
	)
from
	EMPSALARY
;
