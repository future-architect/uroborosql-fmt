SELECT A 
from tb
WHERE
1=22222222222222 --comment0
OR
( -- start
test1 = 1 --comment1
and 
(test2 = 2 --comment2
and 
test3 = 3 --comment3
) --comment4
or (((test4 = 4 --comment5
or test5 = 5 --comment6
)) 
) --comment7
-- end
) --comment8