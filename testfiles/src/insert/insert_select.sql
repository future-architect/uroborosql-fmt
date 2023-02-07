insert into stafflist (name, address) 
  select name, address from newcomer where flag = 'TRUE';