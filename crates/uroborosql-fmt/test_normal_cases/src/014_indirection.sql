-- Indirection (table qualifier)
select
    tbl0.*,
    tbl1.a,
    tbl1.b,
    tbl2.a.c,
    tbl2.a.b.d,
    tbl2.a.b.c as original_c,
    tbl2 . a . b . e,
    tbl2
        .a. b . f
;