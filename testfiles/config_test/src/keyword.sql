select CASE
        when
        a=1
        THEN 
        'one'
        else
        'other'
       END AS	GRADE
from student std
where grade BETWEEN /*start1*/60 AND /*end1*/100 
and grade NOT between /*start2*/70 and /*end2*/80 
;
UPDATE weather SET (temp_lo, temp_hi, prcp) = (temp_lo+1, temp_lo+15, DEFAULT)
  WHERE city = 'San Francisco';
DELETE from products
  WHERE obsoletion_date = 'today'
  RETURNING *;
INSERT into distributors (did, dname) VALUES (default, 'XYZ Widgets')
   RETURNING did;
