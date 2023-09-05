SElECT Identifier  as  				id, --comment1
	student_name          --              comment2
 FROM 		
japanese_student_table  AS 
       JPN_STD --			japanese student
,       
SUBJECT_TABLE AS 
					SBJ  --subject
WHERE           JPN_STD.sportId = 	(SELECT  
         sportId 	FROM Sport WHERE              
Sport.sportname 
    = 		'baseball'
            )   -- 		student playing baseball
    AND JPN_STD.ID  =	
 							SBJ.ID /*IF grade_flag*/							            
AND SBJ.grade   > 
            /*grade*/50 -- grade > 50
/*END*/
		/*IF limit_flag*/ 
limIt 5 
		/*END*/