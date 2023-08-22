SELECT
	*
FROM
/*%if test*/
	EMPLOYEE	EMP1
/*%else*/
	EMPLOYEE	EMP2
/*%end*/
        WHERE
	/*%if test*/
	        EMP.FIRST_NAME	=	/*first_name*/'Bob1'
		/*%if SF.isNotEmpty(first_name)*/
AND	EMP.FIRST_NAME	=	        /*first_name*/'Bob'
		/*%elseif SF.isNotEmpty(last_name1)*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith1'
		/*%elseif SF.isNotEmpty(last_name2)*/
AND	EMP.LAST_NAME	=	
/*last_name*/'Smith2'
		/*%elseif SF.isNotEmpty(last_name3)*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith3'
		/*%else*/
AND	EMP.LAST_NAME	=	/*last_name*/'Smith4'
		/*%end*/
	/*%else*/
	EMP.LAST_NAME	=	/*last_name*/'Smith4'
	/*%end*/
;