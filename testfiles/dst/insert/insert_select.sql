INSERT
INTO
	STAFFLIST
(
	NAME
,	ADDRESS
)
SELECT
	NAME
,	ADDRESS
FROM
	NEWCOMER
WHERE
	FLAG	=	'TRUE'
;