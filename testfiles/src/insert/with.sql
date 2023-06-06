WITH /* _SQL_ID_ */ t-- with句
 AS not materialized( --internal_comment
    SELECT * FROM foo
), --test

t2 (
	a	--カラム1
 ,b	--カラム2
 ,c	--カラム3
 , d	--カラム4
) -- with句
AS ( --internal_comment
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
t3 as materialized( --internal_comment
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
) as (--internal_comment
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
INSERT INTO TABLE1	-- table1
(
COLUMN1	-- col1
, COLUMN2	-- col2
) 
VALUES (VALUE1,VALUE2);

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
INSERT INTO TABLE1	-- table1
(
COLUMN1	-- col1
, COLUMN2	-- col2
) 
VALUES (VALUE1,VALUE2);