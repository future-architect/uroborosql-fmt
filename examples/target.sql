SELECT
      Identifier as id, --ID 
student_name          --              学生名
FROM
  japanese_student_table 
AS JPN_STD --日本人学生
,       SUBJECT_TABLE AS SBJ  --科目
WHERE
  JPN_STD.sportId = (SELECT  
         sportId   FROM
    Sport
                         WHERE              
             Sport.sportname 
    = 'baseball'
                    )   -- 野球をしている生徒
    AND 
JPN_STD.ID  = SBJ.ID            
AND SBJ.grade   > 
            /*grade*/50     --成績が50点以上    