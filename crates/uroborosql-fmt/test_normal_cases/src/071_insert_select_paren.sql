insert into stafflist (name, address, staff_cls) 
  (select name, address,  /*#CLS_STAFF_CLS_NEW_COMER*/'0' from newcomer where flag = 'TRUE');

insert into t (id)
-- comment
-- comment2
select id from t2 where id = 1;
