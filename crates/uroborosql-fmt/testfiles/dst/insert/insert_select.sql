INSERT
INTO
	STAFFLIST
(
	NAME
,	ADDRESS
,	STAFF_CLS
)
SELECT
	NAME							AS	NAME
,	ADDRESS							AS	ADDRESS
,	/*#CLS_STAFF_CLS_NEW_COMER*/'0'
FROM
	NEWCOMER
WHERE
	FLAG	=	'TRUE'
;