-- simple table_function
select * from UNNEST(A) T; 
select * from UNNEST(A) AS T; 
select * from UNNEST(A) WITH ORDINALITY T; 
select * from UNNEST(A) WITH ORDINALITY AS T; 
select * from UNNEST(A)
    WITH        ORDINALITY AS T; 

-- alias with `name_list`
select * from UNNEST(A) WITH ORDINALITY AS T(I, V); 
select * from UNNEST(A) WITH ORDINALITY T(I, V); 

-- alias with `TableFuncElementList`
select * from UNNEST(A) WITH ORDINALITY AS T(I INT, V TEXT); 
select * from UNNEST(A) WITH ORDINALITY T(I INT, V TEXT); 


-- `TableFuncElementList` かつ、 as が省略不可のケース（省略すると構文上不正になりパースできない）
select * from UNNEST(A) WITH ORDINALITY AS (ID INT, V TEXT) -- as は省略されない
;

-- alignment
select * from TEST T1 -- t1
, UNNEST(A) WITH ORDINALITY T2 -- t2
, UNNEST(B) WITH ORDINALITY T3(I INT, V TEXT) -- t3
, UNNEST(C) WITH ORDINALITY T4(I INT -- comment
, V TEXT) -- t4
, UNNEST(D) WITH ORDINALITY TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT5(I INT, V TEXT) -- t5
, UNNEST(E) WITH ORDINALITY -- lhs trailng
T6(I INT, V TEXT) -- t6
, UNNEST(F) WITH ORDINALITY AS (I INT, V TEXT) -- t7
; 

-- comment
select * from UNNEST(A) WITH ORDINALITY AS T_A(I -- comment 1
, V  -- comment 2
)
,
UNNEST(B) WITH ORDINALITY T_B(I -- comment 1
, V  -- comment 2
),
UNNEST(C) WITH ORDINALITY T_C(I INT -- comment 1
, V TEXT  -- comment 2
),
UNNEST(D) WITH ORDINALITY T_D(I INT -- comment 1
, V TEXT  -- comment 2
);
