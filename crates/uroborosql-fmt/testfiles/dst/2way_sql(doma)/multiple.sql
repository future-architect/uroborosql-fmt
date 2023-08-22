SELECT
	*
FROM
/*%if hoge*/
	EMPLOYEE	EMP
/*%elseif huga*/
	STUDENT	STD
/*%elseif foo*/
	TEACHER	TCR
/*%else*/
	PEOPLE	PPL
/*%end*/
WHERE
	EMP.BIRTH_DATE	BETWEEN	/*birth_date_from*/'1990-01-01'	AND	/*birth_date_to*/'1999-12-31'
/*%if SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to)*/
AND	EMP.BIRTH_DATE	BETWEEN	/*birth_date_from*/'1990-01-01'	AND	/*birth_date_to*/'1999-12-31'
/*%elseif SF.isNotEmpty(birth_date_from)*/
AND	EMP.BIRTH_DATE	>=		/*birth_date_from*/'1990-01-01'
/*%else*/
/*%end*/
LIMIT	ALL
OFFSET	5
;