select
	normal_func(COL1	+	COL2, PARAM2)
;
select
	many_args_func(PARAM1, PARAM2, PARAM3, PARAM4)
;
select
	long_args_func(COL1	+	LONGLONGLONGLONGLONGLONGLONG, PARAM2)
;
select
	longlonglonglonglonglonglonglonglonglonglonglong_func(
		PARAM1
	,	PARAM2
	,	PARAM3
	)
;
select
	func1(
		CASE
			WHEN
				Z	=	1
			THEN
				func3(PARAM1, PARAM2, PARAM3, PARAM4, PARAM5)
			ELSE
				func2(
					CASE
						WHEN
							Z	=	1
						THEN
							'ONE'
						ELSE
							func3(PARAM1, PARAM2, PARAM3, PARAM4, PARAM5)
					END
				)
		END
	)
