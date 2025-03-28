select a from t order by a, b;

SELECT * FROM t ORDER BY c USING <;

SELECT
    col
FROM
    tab
ORDER BY
    col ASC -- 昇順
    , long_col DESC NULLS FIRST -- 降順
    , null_col NULLS FIRST -- NULL先
;

SELECT
    *
FROM
    foo t
ORDER BY
    t.bar1
    ,   /* after comma */
    t.bar2
    ,   t.bar3
;

SELECT
    *
FROM
    foo t
ORDER BY
    t.bar1
    /* before comma */
    ,   t.bar2
    ,   t.bar3
;
