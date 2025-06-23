SELECT TRIM('  hello  ') as trimmed1
     , TRIM('abc', 'abcdefabc') as trimmed2
     , TRIM(col1) as trimmed3
     , TRIM(col1, col2) as trimmed4
FROM table1
;
