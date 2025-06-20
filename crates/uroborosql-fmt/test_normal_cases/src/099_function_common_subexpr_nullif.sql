SELECT 
    NULLIF(column1, ''),
    NULLIF(price, 0),
    NULLIF(
        CASE 
            WHEN status = 'active' THEN 'A'
            ELSE 'I'
        END,
        'I'
    ),
    NULLIF(id, parent_id)
;
