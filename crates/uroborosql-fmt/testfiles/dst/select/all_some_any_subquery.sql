select
	*
from
	students
where
	student_id					<>	all	(
		select
			student_id	as	student_id
		from
			exam_results
		where
			student_id	is	not	null
	)
and	longlonglonglonglonglong	=		test
;
select
	*
from
	students
where
	student_id					!=	some	(
		select
			student_id	as	student_id
		from
			exam_results
		where
			student_id	is	not	null
	)
and	longlonglonglonglonglong	=		test
;
select
	*
from
	students
where
	student_id					=	any	(
		select
			student_id	as	student_id
		from
			exam_results
		where
			student_id	is	not	null
	)
and	longlonglonglonglonglong	=		test
;
