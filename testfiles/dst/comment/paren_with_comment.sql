SELECT
	A
FROM
	TB
WHERE
	1	=	22222222222222	-- comment0
OR	(
	-- start
		TEST1	=	1	-- comment1
	AND	(
			TEST2	=	2	-- comment2
		AND	TEST3	=	3	-- comment3
		/* multi comment3 */
		)				-- comment4
	OR	(
			TEST4	=	4	-- comment5
		/*
			multi comment5
		*/
		OR	TEST5	=	5	-- comment6
		)				-- comment7
	-- end
	)						-- comment8
AND	(
		TEST6	=	6	-- comment9
	)
