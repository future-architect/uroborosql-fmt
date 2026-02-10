SELECT
    array[/*a*/1, 2] AS a1
,   array[1, 2 -- t
    ] AS a2
,   array[1 -- x
    , 2] AS a3
,   array[1, 2] -- after_elem
    AS a4
,   array[1, 2] -- after_bracket
    AS a5
;