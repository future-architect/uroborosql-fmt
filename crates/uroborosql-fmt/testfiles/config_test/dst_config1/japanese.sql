SELECT /* _SQL_ID_ */
	"テーブルエイリアス".ID	-- コメント1
								AS	ID				-- コメント2
,	"テーブルエイリアス".COLUMN	AS	JAPANESE_COLUMN	-- コメント3
FROM
	TBL	"テーブルエイリアス"	-- コメント4
WHERE
	1								=	1	-- コメント5
AND	"テーブルエイリアス".ID			=	1	-- コメント6
AND	"テーブルエイリアス"."カラムX"	=	3	-- コメント7
;
