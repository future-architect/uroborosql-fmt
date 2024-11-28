SELECT
	"テーブルエイリアス".id	-- コメント1
								AS	id				-- コメント2
,	"テーブルエイリアス".column	AS	japanese_column	-- コメント3
FROM
	tbl	"テーブルエイリアス"	-- コメント4
WHERE
	1								=	1	-- コメント5
AND	"テーブルエイリアス".id			=	1	-- コメント6
AND	"テーブルエイリアス"."カラムX"	=	3	-- コメント7
;
