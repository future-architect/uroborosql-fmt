select
	a	as	a
from
	tab1
where
	tab1.num											=		1
and	tab1.nuuuuuuuuuuuum									=		2
and	tab1.a												=		3
and	not	(tab1.b	=	5)
and	tab.a	+	func1(tab.s, tab.t)	+	func2(tab.u)	=		2
and	tab.t												is		true
and	tab.f												is		not	false
and	tab.n												is		null
and	tbl.ab												in		(/*param_a*/'A', /*param_b*/'B')
and	tbl.bc												not in	('D', 'E')
