select std.grade as grade 
from student std
where grade between /*start1*/60 and /*end1*/100 -- between
and grade not between /*start2*/70 and /*end2*/80 -- not between