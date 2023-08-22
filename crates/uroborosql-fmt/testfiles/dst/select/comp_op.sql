SELECT
	A	AS	A
FROM
	TAB1
WHERE
	TAB1.NUM											=		1
AND	TAB1.NUUUUUUUUUUUUM									=		2
AND	TAB1.A												=		3
AND	NOT	(TAB1.B	=	5)
AND	TAB.A	+	FUNC1(TAB.S, TAB.T)	+	FUNC2(TAB.U)	=		2
AND	TAB.T												IS		TRUE
AND	TAB.F												IS		NOT	FALSE
AND	TAB.N												IS		NULL
AND	TBL.AB												IN		(/*param_a*/'A', /*param_b*/'B')
AND	TBL.BC												NOT IN	('D', 'E')
