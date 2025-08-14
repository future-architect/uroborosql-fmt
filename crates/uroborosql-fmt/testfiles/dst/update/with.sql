with /* _SQL_ID_ */
	t	-- with句
	as	not materialized	(
		--internal_comment
		select
			*
		from
			foo
	)	-- test
,	t2	as	(
		--internal_comment
		update
			products
		set
			price	=	price	*	1.10
		where
			price	<=	99.99
		returning
			name	as	name
		,	price	as	new_price
	)
,	t3	(
		a	-- カラム1
	,	b	-- カラム2
	,	c	-- カラム3
	,	d	-- カラム4
	)	as	materialized	(
		--internal_comment
		delete
		from
			products
		where
			obsoletion_date	=	'today'
		returning
			*
	)
,	t4	(
		a	-- カラム1
	,	b	-- カラム2
	,	c	-- カラム3
	,	d	-- カラム4
	)	-- with句
	as	(
		--internal_comment
		insert
		into
			distributors
		(
			did
		) values (
			default
		)
		returning
			did	as	did
	)
update
	table1	tbl1	-- テーブル1
set
	tbl1.column2	=	100	-- カラム2
,	tbl1.column3	=	100	-- カラム3
where
	tbl1.column1	=	10
;
with recursive
	t4	as	not materialized	(
		--internal_comment
		insert
		into
			distributors
		(
			did
		) values (
			default
		)
		returning
			did	as	did	-- test
	)	-- comment
update
	table1	tbl1	-- テーブル1
set
	tbl1.column2	=	100	-- カラム2
,	tbl1.column3	=	100	-- カラム3
where
	tbl1.column1	=	10
;
