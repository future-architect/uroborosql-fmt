SELECT
	*
FROM
	EMPLOYEE	EMP
WHERE
	EMP.BIRTH_DATE	BETWEEN	/*birth_date_from*/'1990-01-01'	AND	/*birth_date_to*/'1999-12-31'
/* IF SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to) */
LIMIT	ALL
OFFSET	5
/* END */
;
