select
	tbl.a		as	a		-- aliased
,	tbl.b		as	b		-- complement
,	100						-- number
,	"str"		as	"str"	-- string
,	count(1)				-- count()
from
	tbl
