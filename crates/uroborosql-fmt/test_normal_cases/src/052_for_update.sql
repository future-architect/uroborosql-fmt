SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE
;

SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE OF
    tbl
    , tbl2
;

SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE NOWAIT
;

SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE OF
    tbl
    , tbl2
NOWAIT
; 
