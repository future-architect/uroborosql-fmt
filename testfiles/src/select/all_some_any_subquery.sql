SELECT 	*
FROM 	students
WHERE 	student_id <> ALL
	  (SELECT 	student_id
	   FROM 	exam_results
       WHERE 	student_id IS NOT NULL)
	   AND 
	   longlonglonglonglonglong = test
;

SELECT 	*
FROM 	students
WHERE 	student_id != SOME
	  (SELECT 	student_id
	   FROM 	exam_results
       WHERE 	student_id IS NOT NULL)
	   AND 
	   longlonglonglonglonglong = test
;

SELECT 	*
FROM 	students
WHERE 	student_id = ANY
	  (SELECT 	student_id
	   FROM 	exam_results
       WHERE 	student_id IS NOT NULL)
	   AND 
	   longlonglonglonglonglong = test
;
