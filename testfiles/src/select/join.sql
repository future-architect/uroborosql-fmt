select * from t1 inner join t2 on t1.num = t2.num;
select * from t1 left join t2 on t1.num = t2.num;
select * from t1 inner join t2 on t1.num = t2.num inner join t3 on t2.num = t3.num;
select * from t1 left outer join t2 on t1.num = t2.num;
select * from t1 cross join t2;
select * from t1 natural inner join t2;
select * from t1 -- table 1
cross join t2 -- table 2
;
select * from t1 inner join t2 -- tbl
on t1.num = t2.num -- cond