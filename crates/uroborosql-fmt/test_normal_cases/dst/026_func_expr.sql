-- empty
select
	now()
;
-- Star
select
	count(*)
;
-- multiple args
select
	concat('Hello', ' ', 'World')
;
-- nested
select
	upper(lower('Hello World'))
;
-- expr as arg
select
	abs(-10	+	5)
,	func((a	-	b), c)
;
-- schema func
select
	pg_catalog.current_database()
;
-- aggregate func
select
	count(*)
,	sum(id)
,	avg(id)
from
	users
;
-- column ref and func
select
	name		as	name
,	count(*)
from
	employees
;
-- comments
select
	concat_lower_or_upper(
		'Hello'	-- hello
	,	'World'	-- world
	,	true	-- true
	)
;
