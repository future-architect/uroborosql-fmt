select
coalesce(
min(a) over(
    partition by b
) -- over comment
,0) as a
from t
;
select
coalesce(
count(*) filter (
    where a = 1
) -- filter comment
,0
) as a
from t
;
