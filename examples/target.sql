SELECT
	ID		AS	DATA_ID	-- ID
,	CODE	AS	    DATA_CODE	-- コード
,	NAME	            AS	DATA_NAME	-- 名称
,	VALUE1	AS	VALUE1		-- 値1
,	VALUE2					                            -- 値2
,	(
SELECT
VALUE3
FROM
TABLE2)						-- サブクエリ
FROM
	TABLE1
WHERE
	ID		    =	'DUMMY'	-- IDが'DUMMY'
AND	VAL1	    =	1	-- VAL1が1
AND	CODE =	42	-- CODEが42
OR	VALUE2	=	/* COMMENT */42