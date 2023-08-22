SELECT
	*
FROM
	T
WHERE
	1										=		1
AND	T.AGE_LOOOOOOOOOOOOOOOOOOOOOOOOOOOOOONG	>		10				-- hoge
AND	-- fuga
	T.NAME									LIKE	'%'				-- trailing1
AND	-- foo
	T.NAME									LIKE	'%'	ESCAPE	'$'	-- trailing2
