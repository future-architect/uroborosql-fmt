SELECT /* _SQL_ID_ */
	*
FROM
	TBL	T
WHERE
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
AND	T.AGE	<	100
;
SELECT
	*
FROM
	TBL	T
WHERE
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
OR	T.ID	=	2
;
SELECT
	*
FROM
	TBL	T
WHERE -- comment
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
AND	-- comment
	-- comment
	T.AGE	<	100
;
SELECT
	*
FROM
	TBL	T
WHERE -- comment
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
OR	-- comment
	-- comment
	T.ID	=	2
;
