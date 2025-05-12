select a from t
where (a=1 or a=2
) -- after op 0
-- after op 1
-- before op 2
/*
multi lines: before op 3
*/
and (a=3 or a=4)
;
