select 1 = 1;
select 1 < 2;
select 2 > 1;
select 1 <= 1;
select 1 >= 1;
select 1 <> 2;
select 1 != 2;

-- qual_Op
select 'a' ~ 'a';
select 'a' !~ 'b';
select 'A' ~* 'a';
select 'A' !~* 'b';

select 1 = 1, 2 > 1, 3 <> 4;

select 
1 = 1 as eq,
2 > 1 as gt,
1 < 2 as lt,
1 <= 1 as lte,
1 >= 1 as gte,
1 <> 2 as neq,
1 != 2 as neq2,
'abc' ~ 'a' as regex_match,
'abc' !~ 'd' as regex_not_match,
'ABC' ~* 'a' as regex_match_case_insensitive,
'ABC' !~* 'd' as regex_not_match_case_insensitive;
