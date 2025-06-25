select
count(*) over(partition by
    a, b
    /*IF 1 = 1*/
    ,c
    /*ELIF 1 = 2*/
    ,d,e
    /*END*/
)   as a
from t
;
