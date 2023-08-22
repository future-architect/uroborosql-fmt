SELECT
	*
FROM
	EMPLOYEE	EMP
/*BEGIN*/
WHERE
	EMP.FIRST_NAME	=	/*first_name*/'Bob'
/*IF SF.isNotEmpty(first_name)*/
AND	EMP.FIRST_NAME	=	/*first_name*/'Bob'
/*END*/
/*IF SF.isNotEmpty(last_name)*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith'
/*END*/
/*END*/
;
