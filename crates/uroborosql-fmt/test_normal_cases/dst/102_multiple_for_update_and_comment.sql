select
	*
from
	t1
where
	1	=	1
order by
	t1.id
/*IF 1=1*/
for update of
	t1
nowait
/*ELSE*/
for update of
	t1
/*END*/
;
select
	*
from
	t2
where
	1	=	1
order by
	t2.id
/*IF 1=1*/
for update of
	t2
nowait
-- comment
/*ELSE*/
for update of
	t2
/*END*/
;
