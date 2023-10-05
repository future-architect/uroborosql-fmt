select
	*
from
/*%if test*/
	employee	emp1
/*%else*/
	employee	emp2
/*%end*/
where
/*%if test*/
	emp.first_name	=	/*first_name*/'Bob1'
/*%if SF.isNotEmpty(first_name)*/
and	emp.first_name	=	/*first_name*/'Bob'
/*%elseif SF.isNotEmpty(last_name1)*/
and	emp.last_name	=	/*last_name*/'Smith1'
/*%elseif SF.isNotEmpty(last_name2)*/
and	emp.last_name	=	/*last_name*/'Smith2'
/*%elseif SF.isNotEmpty(last_name3)*/
and	emp.last_name	=	/*last_name*/'Smith3'
/*%else*/
and	emp.last_name	=	/*last_name*/'Smith4'
/*%end*/
/*%else*/
	emp.last_name	=	/*last_name*/'Smith4'
/*%end*/
;