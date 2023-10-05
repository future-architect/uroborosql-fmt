select
	*
from
/*IF test*/
	employee	emp1
/*ELSE*/
	employee	emp2
/*END*/
/*BEGIN*/
where
/*IF test*/
	emp.first_name	=	/*first_name*/'Bob1'
/*IF SF.isNotEmpty(first_name)*/
and	emp.first_name	=	/*first_name*/'Bob'
/*ELIF SF.isNotEmpty(last_name1)*/
and	emp.last_name	=	/*last_name*/'Smith1'
/*ELIF SF.isNotEmpty(last_name2)*/
and	emp.last_name	=	/*last_name*/'Smith2'
/*ELIF SF.isNotEmpty(last_name3)*/
and	emp.last_name	=	/*last_name*/'Smith3'
/*ELSE*/
and	emp.last_name	=	/*last_name*/'Smith4'
/*END*/
/*ELSE*/
	emp.last_name	=	/*last_name*/'Smith4'
/*END*/
/*END*/
;