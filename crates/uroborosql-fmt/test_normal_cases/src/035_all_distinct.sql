-- all keyword
SELECT
    ALL
    itemid
,   itemname
; 

-- distinct keyword
SELECT
    DISTINCT
    itemid
,   itemname
;

-- distinct clause
SELECT
    DISTINCT ON (
        quantity
    ,   itemname
    ,       area
    )
    itemid
,   itemname
;
