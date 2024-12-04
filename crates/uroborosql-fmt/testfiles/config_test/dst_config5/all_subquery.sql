SELECT
	*
FROM
	students
WHERE
		student_id								<>	ALL	(
			SELECT
				student_id	AS	student_id
			FROM
				exam_results
			WHERE
				student_id	IS	NOT	NULL
		)
AND	longlonglonglonglonglong	=				test
;
