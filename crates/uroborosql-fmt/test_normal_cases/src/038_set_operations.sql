-- union
SELECT A
FROM B
/* select - union */
UNION -- union
/* union - subselect */
SELECT C
FROM B 
;
-- intersect
SELECT A
FROM B
/* select - intersect */
INTERSECT -- intersect
/* intersect - subselect */
SELECT C
FROM B 
;
-- except
SELECT A
FROM B
/* select - except */
EXCEPT -- except
/* except - subselect */
SELECT C
FROM B 
;
