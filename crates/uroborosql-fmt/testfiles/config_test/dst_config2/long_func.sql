SELECT
	NORMAL_FUNC(
		col1	+	col2
	,	param2
	)
;
SELECT
	MANY_ARGS_FUNC(
		param1
	,	param2
	,	param3
	,	param4
	)
;
SELECT
	LONG_ARGS_FUNC(
		col1	+	longlonglonglonglonglonglong
	,	param2
	)
;
SELECT
	LONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONG_FUNC(
		param1
	,	param2
	,	param3
	)
;
SELECT
	FUNC1(
		CASE
			WHEN
				z	=	1
			THEN
				FUNC3(
					param1
				,	param2
				,	param3
				,	param4
				,	param5
				)
			ELSE
				FUNC2(
					CASE
						WHEN
							z	=	1
						THEN
							'ONE'
						ELSE
							FUNC3(
								param1
							,	param2
							,	param3
							,	param4
							,	param5
							)
					END
				)
		END
	)
