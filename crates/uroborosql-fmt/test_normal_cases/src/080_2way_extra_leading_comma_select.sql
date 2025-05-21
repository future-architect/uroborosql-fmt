select
,	c1
,	c2
from t
;
select all
,	c1	as	c1
,	c2	as	c2
from
	t
;
select distinct
,	c1	as	c1
,	c2	as	c2
from
	t
;
select distinct on
	(
		c1
	,	c2
	)
,	c1	as	c1
,	c2	as	c2
from
	t
;
