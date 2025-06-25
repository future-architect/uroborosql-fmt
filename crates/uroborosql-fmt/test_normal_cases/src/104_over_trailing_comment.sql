select
coalesce(
min(a) over(
    partition by b
) -- comment
,   f
)   as a
from t
;
