select tbl.a as a -- aliased
, tbl.b -- complement
, 100 -- number
, "column" -- column
, 'str' -- string
, count(1) -- count()
from tbl
