select a from tab1
where tab1.num = 1
and	tbl.ab	in (/*param_a*/'A', /*param_b*/'B') 
and	tbl.ab	in (/*param_a*/'A', --comment 
/*param_b*/'B' -- comment
) 
;
