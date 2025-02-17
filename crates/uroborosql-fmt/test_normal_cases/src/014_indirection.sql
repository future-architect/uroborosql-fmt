-- Indirection (table qualifier)
select
    tbl0.*,
    tbl1.a,
    tbl1.b,
    tbl2.a.c,
    tbl2.a.b.d,
    tbl2.a.b.c as original_c,
    tbl3.a[1],
    tbl3.a[1] as original_a1, 
    tbl3.a[1].e,
    tbl3.a[1].e as original_a1e,
    tbl3.a[1][2].f,
    tbl3.a[1][2].f as original_a12f,
    tbl3.a[1:2],
    tbl3.a[1:2] as original_a12
; 
