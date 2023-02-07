select id, case when grade_point >= 80 then 'A'
    when grade_point < 80 and grade_point >= 70 then 'B'
    when grade_point < 70 and grade_point >= 60 then 'C'
    else 'D' end as grade
from risyu
where subject_number = '005'
