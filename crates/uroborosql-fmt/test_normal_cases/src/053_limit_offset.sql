SELECT
    *
FROM
    T
ORDER BY
    C
    DESC
LIMIT 5
OFFSET 5
;

SELECT
    *
FROM
    T
ORDER BY
    C
    DESC
LIMIT /*$hoge*/5
OFFSET 5
;

SELECT
    *
FROM
    T
ORDER BY
    C
    DESC
LIMIT ALL
OFFSET 5
;

SELECT
    *
FROM
    T
ORDER BY
    C
    DESC
LIMIT /*$hoge*/ALL
OFFSET 5
;

SELECT
    *
FROM
    T
ORDER BY
    C
    DESC
LIMIT 1 + 2
OFFSET 5
;

SELECT
    *
FROM
    T
ORDER BY
    C
    DESC
LIMIT /*$hoge*/100 + 1
OFFSET 5
;
