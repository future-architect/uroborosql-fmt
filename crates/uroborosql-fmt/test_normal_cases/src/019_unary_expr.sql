-- Plus
select +5 as positive_value;
select + 5 as positive_value;

-- Minus
select -10 as negative_value;
select - 10 as negative_value;

-- NOT
select not true as not_true;

-- qual_Op
select ~5 as bitwise_not;
select ~ 5 as bitwise_not;
select |/25 as square_root;
select |/ 25 as square_root;
select ||/8 as cube_root;
select ||/ 8 as cube_root;
-- select @-5 as absolute_value; : PostgreSQLでは、`@-5` と書くと `@-` が一つのトークンとして扱われるため無効なSQLになる
select @ -5 as absolute_value;
