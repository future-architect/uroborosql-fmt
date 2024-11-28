select
	"テーブルエイリアス".ID	-- コメント1
											as	ID				-- コメント2
,	"テーブルエイリアス".column	as	japanese_column	-- コメント3
from
	TBL	"テーブルエイリアス"	-- コメント4
where
	1											=	1	-- コメント5
and	"テーブルエイリアス".ID			=	1	-- コメント6
and	"テーブルエイリアス"."カラムX"	=	3	-- コメント7
;
