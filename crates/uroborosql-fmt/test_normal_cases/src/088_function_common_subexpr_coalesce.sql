SELECT COALESCE(NULL, NULL, 'apple', 'banana', NULL, 'orange');
SELECT
    name,
    COALESCE(description, 'N/A') AS product_description,
    COALESCE(price, 0.00) AS product_price
FROM
    products;
