-- empty
SELECT now();

-- Star
SELECT count(*)
;
-- multiple args
SELECT concat('Hello', ' ', 'World');

-- nested
SELECT upper(lower('Hello World'));

-- expr as arg
SELECT abs(-10 + 5), func((a - b), c);

-- schema func
SELECT pg_catalog.current_database();

-- aggregate func
SELECT count(*), sum(id), avg(id) FROM users;

-- column ref and func
SELECT name, count(*) FROM employees; 

-- comments
SELECT concat_lower_or_upper('Hello'   --hello
,'World'    --world
    ,true    --true
    );
