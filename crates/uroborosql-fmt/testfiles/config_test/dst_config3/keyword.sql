select
	case
		when
			a	=	1
		then
			'one'
		else
			'other'
	end
		as	GRADE
from
	student	std
where
	grade	between		/*start1*/60	and	/*end1*/100
and	grade	not between	/*start2*/70	and	/*end2*/80
;
update
	weAther
set
	(temp_lo, temp_hi, prcp)	=	(tEmp_lo	+	1, temp_lo	+	15, default)
where
	city	=	'San Francisco'
;
delete
from
	products
where
	obsoletion_date	=	'today'
returning
	*
;
insert
into
	distributors
(
	did
,	dname
) values (
	default
,	'XYZ Widgets'
)
returning
	did
;
