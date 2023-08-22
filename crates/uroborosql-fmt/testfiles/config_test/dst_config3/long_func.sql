select
	normal_func(COL1	+	COL2, PARAM2)
;
select
	many_args_func(PARAM1, PARAM2, PARAM3, PARAM4)
;
select
	long_args_func(
		COL1	+	LONGLONGLONGLONGLONGLONGLONG
	,	PARAM2
	)
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
		case
			when
				Z	=	1
			then
				func3(PARAM1, PARAM2, PARAM3, PARAM4, PARAM5)
			else
				func2(
					case
						when
							Z	=	1
						then
							'ONE'
						else
							func3(PARAM1, PARAM2, PARAM3, PARAM4, PARAM5)
					end
				)
		end
	)
