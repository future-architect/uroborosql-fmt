select
	*
from
	employee
where
	id	=	'1'
for update
;
select
	*
from
	employee
where
	id	=	'1'
for update of
	tbl
,	tbl2
;
select
	*
from
	employee
where
	id	=	'1'
for update
nowait
;
select
	*
from
	employee
where
	id	=	'1'
for update of
	tbl
,	tbl2
nowait
;
-- comment after `of`
select
	*
from
	employee
where
	id	=	'1'
for update of
-- comment
	tbl
,	tbl2
nowait
;
-- comments at list of table names
select
	*
from
	employee
where
	id	=	'1'
for update of
	tbl		-- comment 1
-- comment 2
/* comment 3 */
,	tbl2	-- comment 4
nowait
;
-- bind params and comments
select
	*
from
	employee
where
	id	=	'1'
for update of
	/*$param*/tbl	-- comment 1
-- comment 2
/* comment 3 */
,	/*$param*/tbl2	-- comment 4
nowait
;
