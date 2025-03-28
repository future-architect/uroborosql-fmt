select
	id, sum(c)
from
	tbl
	group by
	id
	having 
	sum(c)	>	1
;

select
	id, sum(c)
from
	tbl
	group by
	id
	having 
	sum(c)	>	1 and avg(c)	<	10
;


SELECT
    id
    , sum(c)
FROM
    tbl
GROUP BY
    id
HAVING
    /* comment */
    sum(c) > 1
    AND avg(c) < 10
    AND max(c) <= 100
;
