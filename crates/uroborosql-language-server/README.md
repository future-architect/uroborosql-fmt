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

If the lint config cannot be loaded, the server clears the lint diagnostics of that workspace
and resumes linting once the config loads again.

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

### Catalog Checks

When the lint config has a `db` section, the server also reports tables and columns that do
not exist, using either a PostgreSQL connection (`schemaProvider: "server"`) or a catalog
snapshot exported with `uroborosql-lint export-catalog` (`schemaProvider: "file"`, with
`path` relative to the lint config file's directory). See the
[`uroborosql-lint` CLI README](../uroborosql-lint-cli/README.md) for the `db` settings and
exporting snapshots.

Catalog checks run with the other lint diagnostics on open and save, so a re-exported
snapshot or a database change is picked up on the next open or save. SQL is never executed.

If the catalog cannot be read, the other lint diagnostics are still shown, and the reason is
written to the server log (`window/logMessage`) without connection credentials or SQL text.
When many documents are checked at once, a check may be postponed; saving the document again
retries it.

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
