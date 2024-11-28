SELECT
  id  AS  id
, CASE
    WHEN
      grade_point >=  80
    THEN
      'A'
    WHEN
        grade_point <   80
    AND grade_point >=  70
    THEN
      'B'
    WHEN
        grade_point <   70
    AND grade_point >=  60
    THEN
      'C'
    ELSE
      'D'
  END
   AS  grade
FROM
  risyu
WHERE
  subject_number  = '005'
;
SELECT
  id
, CASE
    grade
    WHEN
      'A'
    THEN
      5
    WHEN
      'B'
    THEN
      4
    WHEN
      'C'
    THEN
      3
    ELSE
      0
  END
   AS  p
FROM
  risyu
WHERE
  subject_number  = '006'
;
SELECT
  CASE
    /*param*/a  -- simple case cond
    WHEN
      /*a*/'a'
    THEN
      'A'
    ELSE
      'B'
  END
