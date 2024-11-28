select
	"テーブルエイリアス".id	-- コメント1
											as	id				-- コメント2
,	"テーブルエイリアス".column	as	japanese_column	-- コメント3
from
	tbl	"テーブルエイリアス"	-- コメント4
where
	1											=	1	-- コメント5
and	"テーブルエイリアス".id			=	1	-- コメント6
and	"テーブルエイリアス"."カラムX"	=	3	-- コメント7
;
