select
	id		as	data_id					-- ID
,	code	as	data_code				-- コード
,	name	as	data_name				-- 名称
,	value1	as	valueaaaaaaaaaaaaaaaa
,	value2	as	value2					-- 値2
,	(
		select
			value3	as	value3
		from
			table2
	)									-- サブクエリ
from
	table1
where
	id		=	'DUMMY'									-- IDが'DUMMY'
and	val1	=	1										-- VAL1が1
and	code	=	42										-- CODEが42
or	value2	=	/*LONGLONGLONGLONG_BIND_PARAMETER*/42
