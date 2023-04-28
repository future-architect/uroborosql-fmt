SELECT
    *
FROM
    department
WHERE
    EXISTS(
        SELECT
            department_id
        FROM
            user
        WHERE
            address = 'TOKYO'
    )
    AND
    test = test