select
	trim('  hello  ')			as	trimmed1
,	trim('abc', 'abcdefabc')	as	trimmed2
,	trim(col1)					as	trimmed3
,	trim(col1, col2)			as	trimmed4
from
	table1
;
