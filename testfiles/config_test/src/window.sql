SELEcT dePname, empno, sAlary,
       rank() OVER (PArTITION bY depname OrDER bY salary DEsC)
FROM empsalary;
-- 0 argument over
SELECT salary -- salary
, sUm(sAlary) OVeR () -- sum
FrOM empsalaRy;
-- frame_clause
SELECT order_id, itEm, qty, 
       SuM(qty) OVeR (OrDER By order_id ROWS BEtWEEN 1 PRECeDING AnD 1 FOLLOwING) result
FROM test_orders;
SELECT *,
	sTring_agg(v, ',') OVeR (
		PARTiTION BY color
        /* partition by */
		ORdER By v
        /* order by */
		GROUPS BeTWEEN UNbOUNDED PREcEDING AnD CURRENT ROw
		EXCLUDE No OTHERS
        /* frame clause with exclusion */
        /* over clause */
	)
	FROM t;
