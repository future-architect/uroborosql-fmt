select
  *
from
  employee  emp
where
/* IF SF.isNotEmpty(birth_date_from) and SF.isNotEmpty(birth_date_to) */
emp.birth_date  between  /*birth_date_from*/'1990-01-01'  and  /*birth_date_to*/'1999-12-31'
/* ELSE */
emp.birth_date  <    /*birth_date_to*/'1999-12-31'
/* END */
;