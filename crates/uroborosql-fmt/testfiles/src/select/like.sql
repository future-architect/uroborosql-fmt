select * from t where 1=1 and t.age_loooooooooooooooooooooooooooooong > 10 -- hoge
 and 
 -- fuga
 t.name like '%' --trailing1
  and
 -- foo
  t.name like '%' escape '$' --trailing2
