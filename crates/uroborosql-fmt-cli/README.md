# uroborosql-fmt-cli

CLI crate for uroborosql-fmt. The installed binary name is `uroborosql-fmt`.

## Install

### Prebuilt binary

Download the binary for your platform from the latest `uroborosql-fmt-cli-v*` release on the
[Releases page](https://github.com/future-architect/uroborosql-fmt/releases):

| Platform | File |
| --- | --- |
| Windows (x64) | `uroborosql-fmt-x86_64-pc-windows-msvc.exe` |
| Windows (ARM64) | `uroborosql-fmt-aarch64-pc-windows-msvc.exe` |
| macOS (Apple silicon) | `uroborosql-fmt-aarch64-apple-darwin` |
| macOS (Intel) | `uroborosql-fmt-x86_64-apple-darwin` |
| Linux (x64) | `uroborosql-fmt-x86_64-unknown-linux-gnu` |

Each file is a standalone executable. Rename it to `uroborosql-fmt` (`uroborosql-fmt.exe`
on Windows) and put it in a directory on your `PATH`. On macOS and Linux, also run
`chmod +x uroborosql-fmt`.

### Build with cargo

Install [Rust](https://www.rust-lang.org/tools/install), then run:

```bash
cargo install --git https://github.com/future-architect/uroborosql-fmt uroborosql-fmt-cli
```

## Usage

```bash
uroborosql-fmt [OPTIONS] [INPUT]
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

For the options available in the configuration file, see
[Configuration options](../../README.md#configuration-options).

### Exit Codes

| Code | Name          | Description                                                                                       |
|------|---------------|---------------------------------------------------------------------------------------------------|
| 0    | `Ok`          | Successful completion                                                                             |
| 1    | `ParseError`  | SQL parsing failed                                                                                |
| 2    | `OtherError`  | Other errors (config file, I/O, formatting differences detected, option conflicts, etc.)          |

## Examples

```bash
# Format a file and output to stdout
uroborosql-fmt query.sql

# Read from STDIN and format
cat query.sql | uroborosql-fmt
uroborosql-fmt < query.sql

# Check formatting differences only
uroborosql-fmt --check query.sql

# Format and overwrite the file
uroborosql-fmt --write query.sql

# Format with a specific configuration file
uroborosql-fmt --config mycfg.json query.sql
```