# debug

Run in debug mode.

Output the following information to standard error output.

- Whether to use the parser error recovery function.
- Whether formatting was done in 2way-sql mode or normal mode.
- Parsing results for postgresql-cst-parser.
- Unique structure generated from postgresql-cst-parser parse results.
- Formatting results for each SQL (only 2way-sql mode).

## Options

- `true`: Debug output at runtime.
- `false` (default): No debug output at runtime.
