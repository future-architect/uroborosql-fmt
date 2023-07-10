SELECT
	*
FROM
/* IF hoge */
	EMPLOYEE	EMP
/* ELIF huga */
	STUDENT	STD
/* ELIF foo */
	TEACHER	TCR
/* ELSE */
	PEOPLE	PPL
/* END */
WHERE
	EMP.BIRTH_DATE	BETWEEN	/*birth_date_from*/'1990-01-01'	AND	/*birth_date_to*/'1999-12-31'
/* IF SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to) */
AND	EMP.BIRTH_DATE	BETWEEN	/*birth_date_from*/'1990-01-01'	AND	/*birth_date_to*/'1999-12-31'
/* ELIF SF.isNotEmpty(birth_date_from) */
AND	EMP.BIRTH_DATE	>=		/*birth_date_from*/'1990-01-01'
/* ELSE */
/* END */
LIMIT	ALL
OFFSET	5
;