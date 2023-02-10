select
	case
		when
			A	=	1
		then
			'one'
		else
			'other'
	end	as	GRADE
from
	STUDENT	STD
where
	GRADE	between		/*start1*/60	and	/*end1*/100
and	GRADE	not between	/*start2*/70	and	/*end2*/80
;
update
	WEATHER
set
	(TEMP_LO, TEMP_HI, PRCP)	=	(TEMP_LO	+	1, TEMP_LO	+	15, DEFAULT)
where
	CITY	=	'San Francisco'
;
delete
from
	PRODUCTS
where
	OBSOLETION_DATE	=	'today'
returning
	*
;
insert
into
	DISTRIBUTORS
(
	DID
,	DNAME
) values (
	DEFAULT
,	'XYZ Widgets'
)
returning
	DID
;
