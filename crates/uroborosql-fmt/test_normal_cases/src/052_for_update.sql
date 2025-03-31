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

-- comment after `of`
SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE OF -- comment
tbl, tbl2 NOWAIT;

-- comments at list of table names
SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE OF
    tbl -- comment 1
    -- comment 2
    /* comment 3 */
    ,tbl2 -- comment 4
NOWAIT
;

-- bind params and comments
SELECT
    *
FROM
    employee
WHERE
    ID = '1'
FOR UPDATE OF
    /*param*/tbl -- comment 1
    -- comment 2
    /* comment 3 */,
    /*param*/tbl2 -- comment 4
NOWAIT
;
