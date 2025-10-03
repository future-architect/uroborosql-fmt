select /* _SQL_ID_ */
	*
from
	tbl
where
	tbl./*$targetColumnName1*/col1									is	null
and	schema1.tbl./*$targetColumnName2*/col2							is	not	null
and	/*$targetColumnName1*/col1	+	tbl./*$targetColumnName2*/col2	=	10
;
