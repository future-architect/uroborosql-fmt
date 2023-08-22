SELECT
	*
FROM
	EMPLOYEE	EMP
WHERE
/*%if SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to)*/
	EMP.BIRTH_DATE	BETWEEN	/*birth_date_from*/'1990-01-01'	AND	/*birth_date_to*/'1999-12-31'
/*%else*/
	EMP.BIRTH_DATE	<	/*birth_date_to*/'1999-12-31'
/*%end*/
;