select * from t1 inner join t2
on t1.num = t2.num and t1.num = t2.num -- trailing
;

select * from t1 inner join t2
on t1.num = t2.numnumnum -- trailing 1
and t1.num = t2.num -- trailing 2
;

select * from
(
    t1 inner join t2
on t1.num = t2.numnumnum -- trailing 1
and t1.num = t2.num -- trailing 2
)
;

select * from
(
    t1 inner join t2
on t1.num = t2.numnumnum -- trailing 1
and t1.num = t2.num -- trailing 2
) as t
;