SELECT
    array[1, 2, 3] AS basic_array
,   array[] AS empty_array
,   array[]::integer[] AS typed_empty_array
,   array[1, 2]::integer[] AS typed_array
,   coalesce(col, array[]::integer[]) AS coalesced_array
,   array['a', 'b', 'c'] AS string_array
,   array[col1, col2, col3] AS column_array;
SELECT array[[1,2],[3,4]];