SELECT
  depname
, empno
, salary
, RANK() OVER(
    PARTITION BY
      depname
    ORDER BY
      salary  DESC
  )
FROM
  empsalary
;
