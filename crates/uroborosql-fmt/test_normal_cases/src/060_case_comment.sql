SELECT a,
        CASE -- case trailing
        /* case */
        WHEN -- cond_1
        a=1 -- a equals 1
        THEN -- cond_1 == true
        'one' -- one
        WHEN -- cond_2
        a=2 -- a equals 2
        THEN -- cond_2 == true
        'two' -- two
        ELSE -- forall i: cond_i == false
        'other' -- other
       END
    FROM test -- test table 
;
