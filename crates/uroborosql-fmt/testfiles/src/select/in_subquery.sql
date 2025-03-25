SELECT
    *
FROM
    department
WHERE
    id IN (
        SELECT
            department_id
        FROM
            users
        WHERE
            address = 'TOKYO'
    )
    AND
    test = test