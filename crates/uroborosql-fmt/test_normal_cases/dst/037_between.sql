-- between
select
	std.grade	as	grade
from
	student	std
where
	grade	between	60	and	70
or	grade	between	80	and	90
;
-- not between
select
	std.grade	as	grade
from
	student	std
where
	grade	between		60	and	100
and	grade	not between	70	and	80
;
-- bind params
select
	std.grade	as	grade
from
	student	std
where
	grade	between		/*start1*/60	and	/*end1*/100
and	grade	not between	/*start2*/70	and	/*end2*/80
;
select
	*
from
	t
where
	1	=		1
and	a	between	/*offset*/0	+	1	and	/*offset*/0	+	/*limit*/10
;
-- comments
select
	std.grade	as	grade
from
	student	std
where
	grade	between		/*start1*/60	and	/*end1*/100	-- between
and	grade	not between	/*start2*/70	and	/*end2*/80	-- not between
;
