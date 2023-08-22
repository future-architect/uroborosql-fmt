SELECT
	*
FROM
/*IF test*/
	EMPLOYEE	EMP1
/*ELSE*/
	EMPLOYEE	EMP2
/*END*/
/*BEGIN*/
        WHERE
	/*IF test*/
	        EMP.FIRST_NAME	=	/*first_name*/'Bob1'
		/*IF SF.isNotEmpty(first_name)*/
AND	EMP.FIRST_NAME	=	        /*first_name*/'Bob'
		/*ELIF SF.isNotEmpty(last_name1)*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith1'
		/*ELIF SF.isNotEmpty(last_name2)*/
AND	EMP.LAST_NAME	=	
/*last_name*/'Smith2'
		/*ELIF SF.isNotEmpty(last_name3)*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith3'
		/*ELSE*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith4'
		/*END*/
	/*ELSE*/
	EMP.LAST_NAME	=	/*last_name*/'Smith4'
	/*END*/
/*END*/
;