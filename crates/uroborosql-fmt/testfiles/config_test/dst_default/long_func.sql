select
	normal_func(col1	+	col2, param2)
;
select
	many_args_func(param1, param2, param3, param4)
;
select
	long_args_func(
		col1	+	longlonglonglonglonglonglong
	,	param2
	)
;
select
	longlonglonglonglonglonglonglonglonglonglonglong_func(
		param1
	,	param2
	,	param3
	)
;
select
	func1(
		case
			when
				z	=	1
			then
				func3(param1, param2, param3, param4, param5)
			else
				func2(
					case
						when
							z	=	1
						then
							'ONE'
						else
							func3(param1, param2, param3, param4, param5)
					end
				)
		end
	)
