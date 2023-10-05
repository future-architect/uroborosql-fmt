select
	*
from
/* IF hoge */
	employee	emp
/* ELIF huga */
	student	std
/* ELIF foo */
	teacher	tcr
/* ELSE */
	people	ppl
/* END */
where
	emp.birth_date	between	/*birth_date_from*/'1990-01-01'	and	/*birth_date_to*/'1999-12-31'
/* IF SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to) */
and	emp.birth_date	between	/*birth_date_from*/'1990-01-01'	and	/*birth_date_to*/'1999-12-31'
/* ELIF SF.isNotEmpty(birth_date_from) */
and	emp.birth_date	>=		/*birth_date_from*/'1990-01-01'
/* ELSE */
/* END */
limit	all
offset	5
;