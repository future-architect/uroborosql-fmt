# uroborosql-lint-cli

CLI crate for `uroborosql-lint`. The installed binary name is `uroborosql-lint`.

## Usage

```bash
uroborosql-lint [OPTIONS] <INPUT>
```

Examples:

```bash
uroborosql-lint query.sql
uroborosql-lint --config .uroborosqllintrc.json query.sql
uroborosql-lint --fail-level warning query.sql
```

### Exit Codes

- `0`: lint succeeded and no diagnostics at or above `--fail-level` were found
- `1`: lint succeeded and at least one diagnostic at or above `--fail-level` was found
- `2`: lint could not complete because of a usage or execution failure such as invalid CLI arguments, invalid config, I/O failure, or SQL parse failure

### Fail Level

Use `--fail-level <none|info|warning|error>` to control which diagnostics cause a non-zero exit code.

- Default: `error`
- `info` currently behaves the same as `warning` because the implemented diagnostics are `warning` or `error` today; it exists so the CLI can stay aligned if `info` diagnostics are added later
- `warning` is useful for CI when warnings, including lint directive warnings, should fail the run
- `none` keeps diagnostics visible without failing the process

For lint rule configuration and directive details, see the [`uroborosql-lint` README](../uroborosql-lint/README.md).
