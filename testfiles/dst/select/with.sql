WITH /* _SQL_ID_ */
	T	-- with句
	AS	NOT MATERIALIZED	(
		--internal_comment
		SELECT
			*
		FROM
			FOO
	)	-- test
,	T2	AS	(
		--internal_comment
		UPDATE
			PRODUCTS
		SET
			PRICE	=	PRICE	*	1.10
		WHERE
			PRICE	<=	99.99
		RETURNING
			NAME	AS	NAME
		,	PRICE	AS	NEW_PRICE
	)
,	T3	(
		A	-- カラム1
	,	B	-- カラム2
	,	C	-- カラム3
	,	D	-- カラム4
	)	AS	MATERIALIZED	(
		--internal_comment
		DELETE
		FROM
			PRODUCTS
		WHERE
			OBSOLETION_DATE	=	'today'
		RETURNING
			*
	)
,	T4	(
		A	-- カラム1
	,	B	-- カラム2
	,	C	-- カラム3
	,	D	-- カラム4
	)	-- with句
	AS	(
		--internal_comment
		INSERT
		INTO
			DISTRIBUTORS
		(
			DID
		) VALUES (
			DEFAULT
		)
		RETURNING
			DID
	)
SELECT
	*
FROM
	T1
;
WITH RECURSIVE
	T4	AS	NOT MATERIALIZED	(
		--internal_comment
		INSERT
		INTO
			DISTRIBUTORS
		(
			DID
		) VALUES (
			DEFAULT
		)
		RETURNING
			DID	-- test
	)	-- comment
SELECT
	*
FROM
	T1
;
