SELECT
	A
FROM
	LONGLONGTABLE	AS	L	-- so long
,	TAB						-- no alias
,	TABLE1			AS	T1	-- normal
,	SOSOSOLONGLONGLONG		-- so long and no alias
WHERE
	L.A						=	L.B							-- normal
AND	SOSOSOLONGLONGLONG.A	=	1							-- so long 
OR	T1.X	+	T1.Y		=	42							-- long lhs
AND	TAB.A					=	1	+	2	+	3	+	5	-- long rhs
