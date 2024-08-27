select
	*
from
	foo	f
order by
/*IF true*/
	f.bar1
,	
/*END*/
	f.bar2
,	f.bar3
select
	*
from
	foo	f
order by
/*IF true*/
	f.bar1
,	
/*END*/
	f.bar2
,	f.bar3
select
	*
from
	foo	f
order by
/*IF true*/
	f.bar1
,	
/*END*/
	-- comment
	f.bar2
,	f.bar3
select
	*
from
	foo	f
order by
/*IF true*/
	f.bar1
,	/*prev*/
/*END*/
	/*next*/
	-- some
	f.bar2
,	f.bar3
