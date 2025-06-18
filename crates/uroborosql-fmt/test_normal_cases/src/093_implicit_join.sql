-- from_list: implicit join alignment
select *
from -- comment after keyword 1
-- comment after keyword 2
table1
,table___2 -- t2_comment
,table3 t3
,table4 t4 -- t4_comment
,
(
    select 1
) t5
,
(
    select 1
) t6 -- t6_comment
,
(
    select * from inner_table1 -- inner_table1_comment
    ,inner_table2 -- inner_table2_comment
) t7
;
