SELECT
    *
FROM
    department
WHERE
    id IN (
        SELECT
            department_id
        FROM
            user
        WHERE
            address = 'TOKYO'
    )
    AND
    test = test