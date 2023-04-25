select
	CASE
		when
			A	=	1
		THEN
			'one'
		else
			'other'
	END
		AS	GRADE
from
	STUDENT	STD
whEre
	GRADE	BETWEEN		/*start1*/60	AND	/*end1*/100
AND	GRADE	NOT BETWEEN	/*start2*/70	AND	/*end2*/80
;
UPDATE
	WEATHER
SET
	(TEMP_LO, TEMP_HI, PRCP)	=	(TEMP_LO	+	1, TEMP_LO	+	15, DEfAULT)
WHeRE
	CITY	=	'San Francisco'
;
DELeTE
from
	PRODUCTS
WHeRE
	OBSOLETION_DATE	=	'today'
RETURNING
	*
;
INSeRT
into
	DISTRIBUTORS
(
	DID
,	DNAME
) VALUES (
	deFault
,	'XYZ Widgets'
)
RETURNING
	DID
;
