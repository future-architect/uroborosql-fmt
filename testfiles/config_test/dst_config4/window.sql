SELEcT
	DEPNAME
,	EMPNO
,	SALARY
,	rank() OVER(
		PArTITION bY
			DEPNAME
		OrDER bY
			SALARY	DEsC
	)
FROM
	EMPSALARY
;
-- 0 argument over
SELECT
	SALARY				-- salary
,	sUm(SALARY) OVeR()	-- sum
FrOM
	EMPSALARY
;
-- frame_clause
SELECT
	ORDER_ID	AS	ORDER_ID
,	ITEM		AS	ITEM
,	QTY			AS	QTY
,	SuM(QTY) OVeR(
		OrDER By
			ORDER_ID
		ROWS	BEtWEEN	1	PRECeDING	AnD	1	FOLLOwING
	)			AS	RESULT
FROM
	TEST_ORDERS
;
SELECT
	*
,	sTring_agg(V, ',') OVeR(
		PARTiTION BY
			COLOR
		/* partition by */
		ORdER By
			V
		/* order by */
		GROUPS	BeTWEEN	UNbOUNDED	PREcEDING	AnD	CURRENT	ROw	EXCLUDE	No	OTHERS
		/* frame clause with exclusion */
		/* over clause */
	)
FROM
	T
;
