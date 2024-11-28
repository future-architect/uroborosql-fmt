SELECT
  *
FROM
  tbl t
WHERE
    t.id  = (
      SELECT
        MAX(t2.id)
      FROM
        tbl t2
    )
AND t.age < 100
;
SELECT
  *
FROM
  tbl t
WHERE
    t.id  = (
      SELECT
        MAX(t2.id)
      FROM
        tbl t2
    )
OR  t.id  = 2
;
SELECT
  *
FROM
  tbl t
WHERE
-- comment
    t.id  = (
      SELECT
        MAX(t2.id)
      FROM
        tbl t2
    )
AND -- comment
    -- comment
    t.age < 100
;
SELECT
  *
FROM
  tbl t
WHERE
-- comment
    t.id  = (
      SELECT
        MAX(t2.id)
      FROM
        tbl t2
    )
OR -- comment
    -- comment
    t.id  = 2
;
