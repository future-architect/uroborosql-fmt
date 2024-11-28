SELECT
	"テーブルエイリアス".ID	-- コメント1
											as	ID				-- コメント2
,	"テーブルエイリアス".COLUMN	as	JAPANESE_COLUMN	-- コメント3
FROM
	TBL	"テーブルエイリアス"	-- コメント4
WHERE
	1											=	1	-- コメント5
and	"テーブルエイリアス".ID			=	1	-- コメント6
and	"テーブルエイリアス"."カラムX"	=	3	-- コメント7
;
