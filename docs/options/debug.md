# debug

Run in debug mode.

Output the following information to standard error output.

- Whether the parser error recovery function is enabled.
- Whether formatting was done in 2way-sql mode or normal mode.
- Parsing results produced by postgresql-cst-parser.
- Unique structure generated from the parse results of postgresql-cst-parser.
- Formatting results for each SQL (2way-sql mode only).

## Options

- `true`: Debug output at runtime.
- `false` (default): No debug output at runtime.
