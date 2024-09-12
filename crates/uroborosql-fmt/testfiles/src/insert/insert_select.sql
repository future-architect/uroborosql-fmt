insert into stafflist (name, address, staff_cls) 
  select name, address,  /*#CLS_STAFF_CLS_NEW_COMER*/'0' from newcomer where flag = 'TRUE';

insert into stafflist (name, address, staff_cls) 
  (select name, address,  /*#CLS_STAFF_CLS_NEW_COMER*/'0' from newcomer where flag = 'TRUE');

insert into tbl (id) 
  select id from tbl2 where id = 1 -- trailing comment
  ;

insert into tbl (id) 
  select id from tbl2 where id = 1 -- trailing comment
  on conflict do nothing
  ;

insert into tbl (id) 
-- comments
-- before select
select id from tbl2 where id = 1;
