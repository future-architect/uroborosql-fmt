# max_char_per_line

If the total number of characters in the function name and arguments exceeds max_char_per_line, the arguments are formatted with new lines.

Default value is 50.

## Example

before:

```sql
SELECT
	NORMAL_FUNC(COL1	+	COL2, PARAM2)
,	LONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONG_FUNC(PARAM1, PARAM2, PARAM3)
```

### max_char_per_line = 100

```sql
SELECT
	NORMAL_FUNC(COL1	+	COL2, PARAM2)
,	LONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONG_FUNC(PARAM1, PARAM2, PARAM3)
```

### max_char_per_line = 50

```sql
SELECT
	NORMAL_FUNC(COL1	+	COL2, PARAM2)
,	LONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONG_FUNC(
		PARAM1
	,	PARAM2
	,	PARAM3
	)
```

### max_char_per_line = 10

```sql
SELECT
	NORMAL_FUNC(
		COL1	+	COL2
	,	PARAM2
	)
,	LONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONGLONG_FUNC(
		PARAM1
	,	PARAM2
	,	PARAM3
	)
```
