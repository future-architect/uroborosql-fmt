select
	*
from
/*%if hoge*/
	employee	emp
/*%elseif huga*/
	student	std
/*%elseif foo*/
	teacher	tcr
/*%else*/
	people	ppl
/*%end*/
where
	emp.birth_date	between	/*birth_date_from*/'1990-01-01'	and	/*birth_date_to*/'1999-12-31'
/*%if SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to)*/
and	emp.birth_date	between	/*birth_date_from*/'1990-01-01'	and	/*birth_date_to*/'1999-12-31'
/*%elseif SF.isNotEmpty(birth_date_from)*/
and	emp.birth_date	>=		/*birth_date_from*/'1990-01-01'
/*%else*/
/*%end*/
limit	all
offset	5
;