select
	a	as	a
,	(
		-- comm1
		/* comm2 */
		select
			z	as	z
		/* z */
		from
			tab2
		/* comm3*/
	)
from
	longlongtable	l
,	(
		select
			b	as	b
		,	c	as	c
		from
			tab1
	)	-- trailing
					bc
