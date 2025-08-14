select
	*
from
	t
order by
	c	desc
limit	5
offset	5
;
select
	*
from
	t
order by
	c	desc
limit	/*$hoge*/5
offset	5
;
select
	*
from
	t
order by
	c	desc
limit	all
offset	5
;
select
	*
from
	t
order by
	c	desc
limit	/*$hoge*/all
offset	5
;
select
	*
from
	t
order by
	c	desc
limit	1	+	2
offset	5
;
select
	*
from
	t
order by
	c	desc
limit	/*$hoge*/100	+	1
offset	5
;
