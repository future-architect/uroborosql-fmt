delete from tbl_a a
using tbl_b b, tbl_c c
where a.col1 = b.col1 and b.col2 > 10 and c.col3 = 'abc';

delete from tbl_a a
using -- comment
-- comment
tbl_b b, tbl_c c
where a.col1 = b.col1 and b.col2 > 10 and c.col3 = 'abc';
