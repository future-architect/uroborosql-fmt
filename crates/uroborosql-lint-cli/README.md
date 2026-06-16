# uroborosql-lint-cli

CLI for `uroborosql-lint`. The installed binary name is `uroborosql-lint`.

## Getting Started

`uroborosql-lint` only runs when a lint config file can be resolved.

### 1. Create a starter config file

```bash
uroborosql-lint --init
```

This creates `.uroborosqllintrc.json` in the current working directory.

### 2. Run lint

```bash
uroborosql-lint query.sql
```

You can also create `.uroborosqllintrc.json` manually or pass a config path with `--config`.

For config file structure, rule settings, and directive details, see the
[`uroborosql-lint` README](../uroborosql-lint/README.md).

## Usage

```bash
uroborosql-lint [OPTIONS] <INPUT>
uroborosql-lint --init
```

Examples:

```bash
uroborosql-lint query.sql
uroborosql-lint --config .uroborosqllintrc.json query.sql
uroborosql-lint --fail-level warning query.sql
uroborosql-lint --init
```

If no lint config can be resolved, the CLI exits with an execution error and prints guidance to
create one.

### Init

Use `uroborosql-lint --init` to create a starter `.uroborosqllintrc.json` in the current working
directory. If the file already exists, the command fails without overwriting it.

### Exit Codes

- `0`: lint succeeded and no diagnostics at or above `--fail-level` were found
- `1`: lint succeeded and at least one diagnostic at or above `--fail-level` was found
- `2`: lint could not complete because of a usage or execution failure such as missing config, invalid CLI arguments, invalid config, I/O failure, or SQL parse failure

### Fail Level

Use `--fail-level <none|info|warning|error>` to control which diagnostics cause a non-zero exit code.

- Default: `error`
- `info` currently behaves the same as `warning` because the implemented diagnostics are `warning` or `error` today; it exists so the CLI can stay aligned if `info` diagnostics are added later
- `warning` is useful for CI when warnings, including lint directive warnings, should fail the run
- `none` keeps diagnostics visible without failing the process
