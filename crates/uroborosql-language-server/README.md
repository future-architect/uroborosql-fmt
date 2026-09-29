# uroborosql-language-server

Language server for `uroborosql-fmt`.

## Overview

`uroborosql-language-server` is the editor-facing entry point for formatting SQL with
`uroborosql-fmt` over LSP.

It provides:

- SQL document formatting
- SQL range formatting
- lint diagnostics when a lint config is available
- quickfix code actions for lint directives
- embedded SQL formatting via a custom request

If you use VS Code, use the dedicated extension:
[`vscode-uroborosql-fmt`](https://github.com/future-architect/vscode-uroborosql-fmt).

If you want to wire the language server into another editor yourself, this README is the starting
point.

## Getting Started

Install the language server:

```sh
cargo install --git https://github.com/future-architect/uroborosql-fmt uroborosql-language-server
```

Run it over stdio:

```sh
uroborosql-language-server
```

### Lint Is Opt-In

Lint diagnostics are published only when the server can resolve a lint config file such as
`.uroborosqllintrc.json`.

Without a lint config file, the server still provides formatting, but it publishes no lint
diagnostics.

To create a starter lint config file, run:

```sh
uroborosql-lint --init
```

See the [`uroborosql-lint` CLI README](../uroborosql-lint-cli/README.md) for lint CLI usage and
config details.

### Current Diagnostic Timing

Lint diagnostics are refreshed when:

- a document is opened
- a document is saved
- workspace folders change
- workspace configuration changes
- watched lint config files change

This server does not currently re-lint on every `textDocument/didChange` notification.

## Editor Setup Examples

These are manual setup examples for people integrating the language server outside VS Code.

### Neovim

Example using Neovim's built-in LSP:

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "sql",
  callback = function(args)
    local root = vim.fs.root(args.buf, {
      ".uroborosqllintrc.json",
      ".uroborosqlfmtrc.json",
    }) or vim.uv.cwd()

    vim.lsp.start({
      name = "uroborosql-language-server",
      cmd = { "uroborosql-language-server" },
      root_dir = root,
    })
  end,
})
```

### Emacs

Example using `Eglot`:

```elisp
(require 'eglot)

(add-to-list 'eglot-server-programs
             '(sql-mode . ("uroborosql-language-server")))

(add-hook 'sql-mode-hook #'eglot-ensure)

(setq eglot-autoshutdown t)
```

## What The Server Supports

Supported LSP methods:

- `textDocument/formatting`
- `textDocument/rangeFormatting`
- `textDocument/codeAction`
- `textDocument/didOpen`
- `textDocument/didChange`
- `textDocument/didSave`
- `textDocument/didClose`
- `workspace/didChangeWorkspaceFolders`
- `workspace/didChangeConfiguration`
- `workspace/didChangeWatchedFiles`

Server notifications:

- `textDocument/publishDiagnostics`

Available code actions:

- add `uroborosql-lint-disable-next-line` directives for lint diagnostics
- remove unknown rule names from existing lint directives

The server does not currently provide features such as completion, hover, or semantic tokens.

## Related Projects

- [`vscode-uroborosql-fmt`](https://github.com/future-architect/vscode-uroborosql-fmt)
- [`uroborosql-fmt` CLI](../uroborosql-fmt-cli/README.md)
- [`uroborosql-lint` CLI](../uroborosql-lint-cli/README.md)

## Protocol Details

For the embedded SQL request, configuration resolution details, and other integration notes, see
[docs/protocol.md](docs/protocol.md).

## Catalog diagnostics

The server uses the lint configuration's `db` settings to check table and column
references on open and save. PostgreSQL support is enabled by default; build with
`--no-default-features --features runtime-tokio` to omit it. SQL is not executed.
The current file-provider configuration reports an unavailable provider; SQLite
integration is maintained separately from the LSP connection.

SQL diagnostics appear through `textDocument/publishDiagnostics`. One
`window/logMessage` summary per accepted analysis reports completed/excluded
statements and acquisition failures. Acquisition failure preserves syntax lint
results and does not turn unknown references into missing-name errors. Connection
credentials and SQL text are not included in these summaries.

Changes invalidate pending results without starting another analysis. Saved
analyses retain their original document snapshot, and stale results after edits,
close/reopen or configuration changes are discarded. Each document has one active
analysis and one pending latest save. At most four analyses acquire definitions
concurrently. Slots are acquired only inside the Provider acquisition call, so
parse errors, disabled catalog rules and unsupported SQL do not wait for a DB slot.
After ten seconds waiting for a slot, catalog acquisition is deferred with an INFO
log; syntax diagnostics are still published. Saving retries the catalog analysis.

Configuration changes suspend new analysis while keeping displayed diagnostics.
Only the latest configuration response can be applied. Configuration acquisition
has a five-second deadline per workspace root. A failed refresh clears that root's
diagnostics and suspends lint until a successful refresh; other roots continue.
Server shutdown stops publication and waits at most five seconds for active work.

The protocol regression tests use controlled providers and configuration responses.
The ignored `postgres_configuration_reaches_lsp_diagnostics` test additionally
requires PostgreSQL with `public.users(id)`, user `postgres`, password
`catalog-test`, database `postgres`, and the port in `LSP_TEST_PG_PORT`.

The ignored `shutdown_releases_real_postgres_session_and_transaction` test is only
for a disposable local PostgreSQL container, never an existing database. In addition
to `LSP_TEST_PG_PORT`, it requires `LSP_TEST_DISPOSABLE_DATABASE` to be explicitly
set to `catalog-lsp-smoke-20260929`. It temporarily renames a catalog privilege
function and installs a same-signature advisory-lock fixture, restoring the original
function on success. Always discard the test container on failure. The test first
proves the fixture blocks standalone SQL, then observes a real Provider transaction
waiting for that lock, invokes shutdown, and checks from another connection that
the Provider session is gone while the lock is still held. The fixture role uses
`client_connection_check_interval=100ms` to make server-side disconnect observation
bounded; this is a test setting, not a change to user database configuration.
