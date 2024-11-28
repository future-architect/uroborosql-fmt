select
123456789 -- hoge
 as 
 col
from 
tbl t;
select
1 -- hoge
 as 
 col1
,123456789-- fuga 
 as 
 col2
from 
tbl t;
SELECT a,
    CASE -- case trailing
    /* case */
    WHEN -- cond_1
    a=1 -- a equals 1
    THEN -- cond_1 == true
    'one' -- one
    WHEN -- cond_2
    a=2 -- a equals 2
    THEN -- cond_2 == true
    'two' -- two
    ELSE -- forall i: cond_i == false
    'other' -- other
    END -- comment
    as COL
FROM test -- test table
select
123456789 -- hoge
 col
from 
tbl t;
select
1 -- hoge
 col1
,123456789-- fuga 
 col2
from 
tbl t;
SELECT a,
    CASE -- case trailing
    /* case */
    WHEN -- cond_1
    a=1 -- a equals 1
    THEN -- cond_1 == true
    'one' -- one
    WHEN -- cond_2
    a=2 -- a equals 2
    THEN -- cond_2 == true
    'two' -- two
    ELSE -- forall i: cond_i == false
    'other' -- other
    END -- comment
     COL
FROM test -- test table
where    CASE
    WHEN
    a=1
    THEN 
    'one'
    ELSE
    'other'
    END =  CASE
    WHEN
    a=1
    THEN 
    'one'
    ELSE
    'other'
    END 
;
