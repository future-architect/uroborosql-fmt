select id, case when grade_point >= 80 then 'A'
    when grade_point < 80 and grade_point >= 70 then 'B'
    when grade_point < 70 and grade_point >= 60 then 'C'
    else 'D' end as grade
from risyu
where subject_number = '005';
select id, case grade
    when 'A' then 5
    when 'B' then 4
    when 'C' then 3
    else 0 end as p

from risyu
where subject_number = '006';
select case /*param*/a -- simple case cond
    when /*a*/'a' then 'A'
    else 'B'
    end