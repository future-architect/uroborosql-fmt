select 
    id,
    category,
    subcategory,
    region,
    value,
    row_number() over (partition by category, subcategory, region order by value) as rn
from test_table;
