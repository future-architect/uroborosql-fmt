select tbl.a as a -- aliased
, tbl.b -- complement
, 100 -- number
, "str" -- string
, count(1) -- count()
from tbl
