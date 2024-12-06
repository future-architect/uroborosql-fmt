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
