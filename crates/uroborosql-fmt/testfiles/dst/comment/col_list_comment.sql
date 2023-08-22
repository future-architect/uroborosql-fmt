SELECT
	*
FROM
	TBL
WHERE
	TBL.A	IN	(
		'AAA'	-- a
	,	'bbbbb'	-- b
	,	'c'		-- c
	)
;
SELECT
	*
FROM
	TBL
WHERE
	TBL.AB	IN	(/*var_a*/'A', /*var_b*/'B')
;
SELECT
	*
FROM
	TBL
WHERE
	TBL.XY	IN	/*var*/('X', 'Y')
;
SELECT
	*
FROM
	TBL
WHERE
	TBL.XY	IN	(/*var_a*/'A', /*var_b*/'B')	-- ab
AND	TBL.XY	IN	/*var*/('X', 'Y')				-- xy
AND	TBL.ST	IN	(
		'S'	-- s
	,	'T'	-- t
	)											-- st
;
