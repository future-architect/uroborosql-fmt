select
	*
from
	tbl1
join
-- after keyword
/* block comment between join and table_ref */
	/*$tableName*/tbl2
on
	tbl1.id	=	tbl2.id
