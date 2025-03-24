select
	+5	as	positive_value
;
select
	+5	as	positive_value
;
select
	-10	as	negative_value
;
select
	-10	as	negative_value
;
select
	not	true	as	not_true
;
select
	~5	as	bitwise_not
;
select
	~5	as	bitwise_not
;
select
	|/25	as	square_root
;
select
	|/25	as	square_root
;
-- select @-5 as absolute_value; : PostgreSQLでは、`@-5` と書くと `@-` が一つのトークンとして扱われるため無効なSQLになる
select
	@-5	as	absolute_value
;
select
	||/8	as	cube_root
;
select
	||/8	as	cube_root
;
-- 単項演算子がある場合の縦揃え
select
	1		as	positive_value	-- 式
,	-2		as	negative_value	-- 1文字
,	|/4		as	square_root		-- 2文字
,	||/8	as	cube_root		-- 3文字
;
