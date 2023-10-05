select
	*
from
	employee	emp
where
	emp.birth_date	between	/*birth_date_from*/'1990-01-01'	and	/*birth_date_to*/'1999-12-31'
/* IF SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to) */
limit	all
offset	10
/* ELIF SF.isNotEmpty(birth_date_from) */
limit	all
offset	5
/* END */
;