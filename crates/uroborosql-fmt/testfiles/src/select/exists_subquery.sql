SELECT
    *
FROM
    department
WHERE
    EXISTS(
        SELECT
            department_id
        FROM
            users
        WHERE
            address = 'TOKYO'
    )
    AND
    test = test