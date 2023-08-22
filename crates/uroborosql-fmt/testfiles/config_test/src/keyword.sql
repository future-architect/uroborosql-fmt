select CASE
        when
        a=1
        THEN 
        'one'
        else
        'other'
       END AS	GRADE
from student std
whEre grade BeTWEEN /*start1*/60 AND /*end1*/100 
and grade NOT between /*start2*/70 and /*end2*/80 
;
UPDATE weAther SET (temp_lo, temp_hi, prcp) = (tEmp_lo+1, temp_lo+15, DEfAULT)
  WHeRE city = 'San Francisco';
DELeTE from products
  WHeRE obsoletion_date = 'today'
  RETURNING *;
INSeRT into distributors (did, dname) VALUES (deFault, 'XYZ Widgets')
   RETURNING did;
