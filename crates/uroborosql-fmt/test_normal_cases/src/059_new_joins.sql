-- triple join
select * 
from
    t1 inner join t2 on t1.id = t2.id
    inner join t3 on t2.id = t3.id;

-- join with alias
select
    *
from
(
    t1 inner join t2 on t1.id = t2.id
) as t1_t2;

-- join with alias (2)
select
    *
from
(
    tbl1 t1 inner join tbl2 t2 on t1.id = t2.id
) as t1_t2;

-- join with alias, with comments
select
    *
from
(
    tbl1 t1 -- left comment
    inner join 
    tbl2 t2 -- right comment
    on -- on comment
    -- on comment 2
    t1.id = t2.id  -- tail comment
    -- end comment
) as t1_t2;

-- old cross join
select * from t1, t2, t3;

-- after join keyword comment
select
	*
from
	(
		select
			*
		from
			t1
		cross join
		-- comment 1
		-- comment 2
			t2
	)	a
;
