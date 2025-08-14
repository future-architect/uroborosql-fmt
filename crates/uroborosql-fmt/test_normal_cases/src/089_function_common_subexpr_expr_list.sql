-- COALESCE function tests
SELECT COALESCE(NULL, NULL, 'default');
SELECT COALESCE(column_a, column_b, 'fallback') FROM table_name;
SELECT COALESCE(1, 2, 3, 4, 5);

-- GREATEST function tests  
SELECT GREATEST(10, 5, 20, 15);
SELECT GREATEST('apple', 'banana', 'orange');
SELECT GREATEST('2023-01-15'::date, '2023-03-10'::date, '2022-12-25'::date);
SELECT GREATEST(10, NULL, 20, 5);

-- LEAST function tests
SELECT LEAST(10, 5, 20, 15);
SELECT LEAST('apple', 'banana', 'orange');
SELECT LEAST('2023-01-15'::date, '2023-03-10'::date, '2022-12-25'::date);
SELECT LEAST(10, NULL, 20, 5);

-- XMLCONCAT function tests
SELECT XMLCONCAT('<tag1>Value 1</tag1>', '<tag2>Value 2</tag2>');
SELECT XMLCONCAT('<a>Text A</a>', '<b>Text B</b>', '<c>Text C</c>');
SELECT XMLCONCAT(xml_column1, xml_column2) FROM xml_table;

-- Complex query with multiple expr_list functions
SELECT
    product_name,
    COALESCE(price_a, price_b, 0) AS coalesced_price,
    GREATEST(price_a, price_b, price_c) AS highest_price,
    LEAST(price_a, price_b, price_c) AS lowest_price,
    XMLCONCAT('<product>', product_name, '</product>') AS xml_product
FROM
    product_prices; 