select a from t -- comment
;

select a from t.a -- comment
;

SELECT u.* FROM test_schema
    .users -- comment
;

select
    a
from longlongtable as l -- so long
, tab -- no alias
, table1 as t1 -- normal
, sososolonglonglong -- so long and no alias
;

select * from -- comment
t;
