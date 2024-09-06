select
	*
from
	tbl
where
	tbl.a	in	(
		'AAA'	-- a
	,	'bbbbb'	-- b
	,	'c'		-- c
	)
;
select
	*
from
	tbl
where
	tbl.ab	in	(/*var_a*/'A', /*var_b*/'B')
;
select
	*
from
	tbl
where
	tbl.xy	in	/*var*/('X', 'Y')
;
select
	*
from
	tbl
where
	tbl.xy	in	(/*var_a*/'A', /*var_b*/'B')	-- ab
and	tbl.xy	in	/*var*/('X', 'Y')				-- xy
and	tbl.st	in	(
		'S'	-- s
	,	'T'	-- t
	)											-- st
;
select
	*
from
	tbl	t
where
	t.id	in	(
		-- after opening paren
		-- another comment
		/*firstId*/0
	,	/*secondId*/1
	)
;
