-- LATERAL JOIN with subquery
SELECT
    c.category_name,
    p.product_name,
    p.created_at
FROM
    categories AS c
LEFT OUTER JOIN LATERAL (
    SELECT
        pr.product_name,
        pr.created_at
    FROM
        products AS pr
    WHERE
        pr.category_id = c.id
    ORDER BY
        pr.created_at DESC
    LIMIT 3
) AS p ON TRUE;

-- LATERAL with INNER JOIN
SELECT
    t1.id,
    t2.value
FROM
    t1
INNER JOIN LATERAL (
    SELECT * FROM t2 WHERE t2.t1_id = t1.id
) AS t2 ON TRUE;

-- LATERAL with CROSS JOIN
SELECT
    t1.id,
    t2.value
FROM
    t1
CROSS JOIN LATERAL (
    SELECT * FROM t2 WHERE t2.t1_id = t1.id
) AS t2;

-- LATERAL without AS keyword
SELECT
    c.name,
    p.product
FROM
    categories c
LEFT OUTER JOIN LATERAL (
    SELECT product FROM products WHERE category_id = c.id LIMIT 1
) p ON TRUE;

-- Multiple LATERAL JOINs
SELECT
    a.id,
    b.val,
    c.val
FROM
    table_a a
LEFT OUTER JOIN LATERAL (
    SELECT val FROM table_b WHERE a_id = a.id LIMIT 1
) AS b ON TRUE
LEFT OUTER JOIN LATERAL (
    SELECT val FROM table_c WHERE a_id = a.id LIMIT 1
) AS c ON TRUE;
