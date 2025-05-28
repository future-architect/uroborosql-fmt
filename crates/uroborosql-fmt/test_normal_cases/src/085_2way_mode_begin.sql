select
  *
from
  employee  emp
/*BEGIN*/
where
emp.first_name  =  /*first_name*/'Bob'
/*IF SF.isNotEmpty(first_name)*/
and  emp.first_name  =  /*first_name*/'Bob'
/*END*/
/*IF SF.isNotEmpty(last_name)*/
and  emp.last_name  =  /*last_name*/'Smith'
/*END*/
/*END*/
;
