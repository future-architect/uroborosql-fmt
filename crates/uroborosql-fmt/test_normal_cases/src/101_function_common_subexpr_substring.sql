SELECT 
    SUBSTRING('Hello World', 1, 5) as basic_substring,
    SUBSTRING(column_name, 2, 10) as column_substring,
    SUBSTRING('test string', 6) as substring_from_position,
    SUBSTRING(CONCAT(first_name, ' ', last_name), 1, 20) as complex_substring,
    SUBSTRING(description, LENGTH(description) - 10) as end_substring
FROM users
WHERE SUBSTRING(email, 1, 5) = 'admin'
;
