select
	a	as	a
from
	tb
where
	1	=	22222222222222	-- comment0
or	(
	-- start
		test1	=	1	-- comment1
	and	(
			test2	=	2	-- comment2
		and	test3	=	3	-- comment3
		/* multi comment3 */
		)				-- comment4
	or	(
			test4	=	4	-- comment5
		/*
			multi comment5
		*/
		or	test5	=	5	-- comment6
		)				-- comment7
	-- end
	)						-- comment8
and	(
		test6	=	6	-- comment9
	)
