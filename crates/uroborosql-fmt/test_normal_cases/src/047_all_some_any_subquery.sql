select 	*
from 	tbl1
where 	col1 != all
	  (select 	col1
	   from 	tbl2
       where 	col1 is not null)
	   and 
	   longlonglonglonglonglong = test
;

select 	*
from 	tbl1
where 	col1 != some
	  (select 	col1
	   from 	tbl2
       where 	col1 is not null)
	   and 
	   longlonglonglonglonglong = test
;

select 	*
from 	tbl1
where 	col1 = any
	  (select 	col1
	   from 	tbl2
       where 	col1 is not null)
	   and 
	   longlonglonglonglonglong = test
;
