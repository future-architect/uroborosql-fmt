SELECT id, avg(grade) FROM student GROUP BY id;
SELECT concat_lower_or_upper('Hello',
    'World',
    true);
SELECT func(CASE WHEN flag THEN a ELSE b, c);
SELECT city, max(temp_lo)
    FROM weather
    GROUP BY city
    HAVING max(temp_lo) < 40;
SELECT func((a - b), c)
