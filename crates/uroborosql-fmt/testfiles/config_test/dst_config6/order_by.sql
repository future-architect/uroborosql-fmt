SELECT
  col
FROM
  tab
ORDER BY
  col       ASC               -- 昇順
, long_col  DESC NULLS FIRST  -- 降順
, null_col  NULLS FIRST       -- NULL先
