select * from employee
where ID = '1'
for update;

select * from employee
where ID = '1'
for update of tbl, tbl2;

select * from employee
where ID = '1'
for update nowait;

select * from employee
where ID = '1'
for update of tbl, tbl2 nowait;