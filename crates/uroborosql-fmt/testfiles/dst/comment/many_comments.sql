/* discription */
-- hoge
select /* _SQL_ID_ */ -- after keyword, and on the same line as SQL_ID
/* select body */
-- comment
	std.id		as	id		-- identifier
,	std.grade	as	grade
-- single line comment
/*
    multi lines comment
    hoge hoge fuga
*/
,	std.age		as	age		-- age
/* end select */
-- from clause
from
/* table lists */
	student		std
,	professor	prof
where
/* conditions */
	id		=	5	-- check id
/* others */
and	age		>=	18
/* hoge */
/* huga */
and	-- this comment follows "AND"
	grade	>	50
