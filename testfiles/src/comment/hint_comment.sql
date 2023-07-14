select 
/*+ 
    FULL(c) FULL(b) FULL(a) LEADING(a b c) USE_HASH(b c) 
*/ *
from departments a,
    employees b,
    locations c
where a.manager_id = b.manager_id
and a.location_id = c.location_id;