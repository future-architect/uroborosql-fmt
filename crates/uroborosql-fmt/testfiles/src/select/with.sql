WITH/* _SQL_ID_ */ t -- with句
AS not materialized( --internal_comment
    SELECT * FROM foo -- foo
	-- end
), --test

t2 AS ( --internal_comment
UPDATE
	PRODUCTS
SET
	PRICE	=	PRICE	*	1.10
WHERE
	PRICE	<=	99.99
RETURNING
	NAME	AS	NAME
,	PRICE	AS	NEW_PRICE


),
t3(
	a	--カラム1
 ,b	--カラム2
 ,c	--カラム3
 , d	--カラム4
)  as materialized ( --internal_comment
    DELETE
FROM
	products
WHERE
	obsoletion_date	=	'today'
RETURNING
	*
),
t4 (
	a	--カラム1
 ,b	--カラム2
 ,c	--カラム3
 , d	--カラム4
) -- with句
as (--internal_comment
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
SELECT * FROM t1;


with recursive t4 as not materialized (--internal_comment
    INSERT
INTO
	DISTRIBUTORS
(
	DID
) VALUES (
	DEFAULT
)
RETURNING
	DID 	-- test
) --comment
SELECT * FROM t1;

with recursive /* _SQL_ID_ */ /* block */ -- line
	t	as	(
		select
			1
	)
select
	1;
