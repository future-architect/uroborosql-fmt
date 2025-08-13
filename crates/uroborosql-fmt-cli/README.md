# uroborosql-fmt-cli

## Install

```bash
cargo install --git https://github.com/future-architect/uroborosql-fmt
```

## Usage

```bash
uroborosql-fmt-cli -- [OPTIONS] [INPUT]
```

### Arguments

* `INPUT` — Path to the SQL file to format.
  * If omitted, reads from STDIN.

### Options

| Option                    | Description                                                                                              |
|---------------------------|----------------------------------------------------------------------------------------------------------|
| `-w`, `--write`           | Overwrite the `INPUT` file with the formatted result. Cannot be used with `--check`.                     |
| `-c`, `--check`           | Exit with code 2 if formatting changes are detected. Cannot be used with `--write`.                     |
| `--config <FILE>`         | Specify the configuration file path. Defaults to `.uroborosqlfmtrc.json` in the current directory.      |
| `-h`, `--help`            | Print help information.                                                                                  |
| `-V`, `--version`         | Print version information.                                                                               |

### Exit Codes

| Code | Name          | Description                                                                                       |
|------|---------------|---------------------------------------------------------------------------------------------------|
| 0    | `Ok`          | Successful completion                                                                             |
| 1    | `ParseError`  | SQL parsing failed                                                                                |
| 2    | `OtherError`  | Other errors (config file, I/O, formatting differences detected, option conflicts, etc.)          |

## Examples

```bash
# Format a file and output to stdout
uroborosql-fmt-cli query.sql

# Read from STDIN and format
cat query.sql | uroborosql-fmt-cli
uroborosql-fmt-cli < query.sql

# Check formatting differences only
uroborosql-fmt-cli --check query.sql

# Format and overwrite the file
uroborosql-fmt-cli --write query.sql

# Format with a specific configuration file
uroborosql-fmt-cli --config mycfg.json query.sql
```