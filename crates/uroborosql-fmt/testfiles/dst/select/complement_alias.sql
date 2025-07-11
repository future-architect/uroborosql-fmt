select
	tbl.a		as	a			-- aliased
,	tbl.b		as	b			-- complement
,	100							-- number
,	"column"	as	"column"	-- column
,	'str'						-- string
,	count(1)					-- count()
from
	tbl
