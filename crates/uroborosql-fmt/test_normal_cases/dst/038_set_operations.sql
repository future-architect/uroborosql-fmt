-- union
select
	a	as	a
from
	b
/* select - union */
union
-- union
/* union - subselect */
select
	c	as	c
from
	b
;
-- intersect
select
	a	as	a
from
	b
/* select - intersect */
intersect
-- intersect
/* intersect - subselect */
select
	c	as	c
from
	b
;
-- except
select
	a	as	a
from
	b
/* select - except */
except
-- except
/* except - subselect */
select
	c	as	c
from
	b
;
