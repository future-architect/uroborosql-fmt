select
	*
from
	TBL	T
where
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
and	T.AGE	<	100
;
select
	*
from
	TBL	T
where
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
or	T.ID	=	2
;
select
	*
from
	TBL	T
where
-- comment
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
and	-- comment
	-- comment
	T.AGE	<	100
;
select
	*
from
	TBL	T
where
-- comment
	T.ID	=	(
		SELECT
			MAX(T2.ID)
		FROM
			TBL	T2
	)
or	-- comment
	-- comment
	T.ID	=	2
;
