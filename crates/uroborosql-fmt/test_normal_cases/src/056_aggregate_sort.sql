SELECT
	string_agg(DISTINCT tbl.column1, ',' ORDER BY tbl.column2, tbl.column3); 
