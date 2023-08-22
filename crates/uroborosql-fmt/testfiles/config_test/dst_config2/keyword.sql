SELECT
	CASE
		WHEN
			a	=	1
		THEN
			'one'
		ELSE
			'other'
	END
		AS	grade
FROM
	student	std
WHERE
	grade	BETWEEN		/*start1*/60	AND	/*end1*/100
AND	grade	NOT BETWEEN	/*start2*/70	AND	/*end2*/80
;
UPDATE
	weather
SET
	(temp_lo, temp_hi, prcp)	=	(temp_lo	+	1, temp_lo	+	15, DEFAULT)
WHERE
	city	=	'San Francisco'
;
DELETE
FROM
	products
WHERE
	obsoletion_date	=	'today'
RETURNING
	*
;
INSERT
INTO
	distributors
(
	did
,	dname
) VALUES (
	DEFAULT
,	'XYZ Widgets'
)
RETURNING
	did
;
