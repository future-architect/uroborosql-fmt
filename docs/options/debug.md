# debug

Run in debug mode.

Output the following information to standard error output.

- Whether formatting was done in 2way-sql mode or normal mode.
- Parsing results for tree-sitter-sql.
- Unique structure generated from tree-sitter-sql parse results.
- Formatting results for each SQL (only 2way-sql mode).

## Options

- `true`: Debug output at runtime.
- `false` (default): No debug output at runtime.
