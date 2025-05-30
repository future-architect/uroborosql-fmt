SELECT GREATEST(10, 5, 20, 15);
SELECT GREATEST('apple', 'banana', 'orange');
SELECT GREATEST('2023-01-15'::date, '2023-03-10'::date, '2022-12-25'::date);
SELECT GREATEST(10, NULL, 20, 5);
SELECT GREATEST(NULL, NULL, NULL);
SELECT LEAST(10, 5, 20, 15);
SELECT LEAST('apple', 'banana', 'orange');
SELECT LEAST('2023-01-15'::date, '2023-03-10'::date, '2022-12-25'::date);
SELECT LEAST(10, NULL, 20, 5);
SELECT LEAST(NULL, NULL, NULL);
SELECT
    product_name,
    GREATEST(price_a, price_b, price_c) AS highest_price,
    LEAST(price_a, price_b, price_c) AS lowest_price
FROM
    product_prices;
