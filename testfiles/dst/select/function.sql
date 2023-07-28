SELECT
	ID			AS	ID
,	AVG(GRADE)
FROM
	STUDENT
GROUP BY
	ID
;
SELECT
	CONCAT_LOWER_OR_UPPER(
		'Hello'	-- hello
	,	'World'	-- world
	,	TRUE	-- true
	)
;
SELECT
	FUNC(
		CASE
			WHEN
				FLAG
			THEN
				A
			ELSE
				B
		END
	,	C
	)
;
SELECT
	CITY			AS	CITY
,	MAX(TEMP_LO)
FROM
	WEATHER
GROUP BY
	CITY
HAVING
	MAX(TEMP_LO)	<	40
;
SELECT
	FUNC((A	-	B), C)
