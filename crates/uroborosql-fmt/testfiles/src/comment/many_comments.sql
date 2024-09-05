/* discription */
-- hoge
SELECT /* _SQL_ID_ */ -- after keyword, and on the same line as SQL_ID
/* select body */ -- comment
    STD.ID AS ID -- identifier
,    STD.GRADE AS GRADE
    -- single line comment
    /* 
        multi lines comment 
        hoge hoge fuga
    */
,    STD.AGE AS AGE -- age
/* end select */
-- from clause
FROM /* table lists */
STUDENT STD
, PROFESSOR PROF
WHERE /* conditions */
ID = 5 -- check id
/* others */
and age >= 18
/* hoge */
/* huga */
and -- this comment follows "AND"
    grade > 50