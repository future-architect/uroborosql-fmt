select
	array[/*a*/1, 2]	as	a1
,	array[
		1
	,	2	-- t
	]					as	a2
,	array[
		1	-- x
	,	2
	]					as	a3
,	array[1, 2]	-- after_elem
						as	a4
,	array[1, 2]	-- after_bracket
						as	a5
;
