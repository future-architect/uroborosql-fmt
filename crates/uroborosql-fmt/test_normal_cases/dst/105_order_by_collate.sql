select
	*
from
	multilingual_test
order by
	japanese_text	collate	/*$LC_COLLATE*/"ja_JP.UTF-8"	desc
,	german_text	collate	"de_DE.UTF-8"
;
