select
  std.id as id, -- identifier
  std.grade -- students grade
from
  student std left join subject sbj
