# uroborosql-language-server

Language server for `uroborosql-fmt`.

## Overview

`uroborosql-language-server` provides:

- SQL document formatting
- SQL range formatting
- diagnostics when a lint config is available
- embedded SQL formatting via a custom request

The server only publishes lint diagnostics when it can resolve a lint config file such as `.uroborosqllintrc.json`.

## Supported LSP methods

- `textDocument/formatting`
- `textDocument/rangeFormatting`
- `textDocument/didOpen`
- `textDocument/didChange`
- `textDocument/didSave`
- `textDocument/didClose`
- `workspace/didChangeConfiguration`
- `workspace/didChangeWatchedFiles`

## Server notifications

- `textDocument/publishDiagnostics`

## Custom requests

### `uroborosql/formatSelectionsAsSql`

### Request

```ts
type FormatSelectionsAsSqlParams = {
  hostDocumentUri: string;
  hostDocumentVersion: number;
  selections: {
    range: Range;
    text: string;
  }[];
};
```

#### Field semantics

- `hostDocumentUri`
  - Required.
  - URI of the host document that owns the selections.
  - The language server resolves formatter configuration using this URI as the configuration scope.
- `hostDocumentVersion`
  - Required.
  - Version of the host document when the client collected the selections.
  - The server echoes this value in the response so clients can detect stale results before applying edits.
- `selections`
  - Required.
  - One or more independent SQL fragments to format.
  - Each item contains:
    - `range`: the range to replace in the host document
    - `text`: the exact selected text to format as SQL

### Response

```ts
type FormatSelectionsAsSqlResult = {
  hostDocumentVersion: number;
  edits: {
    range: Range;
    newText: string;
  }[];
};
```

#### Response semantics

- `hostDocumentVersion`
  - Echo of the request field.
  - Clients should compare this value with the current document version before applying edits.
- `edits`
  - Replacement edits in the same order as the request `selections`.
  - Each `range` matches the corresponding input selection range.
  - Each `newText` contains the formatted SQL fragment for that selection.

### Error handling

- `selections` must not be empty. An empty list is rejected as invalid params.
- The request is intentionally all-or-nothing.
  - If formatting any single selection fails, the server returns an error for the whole request.
  - The server does not return partial success.
  - This allows clients to avoid reconciling partially formatted multi-selection edits.

### Configuration resolution

- Formatter configuration is resolved using `hostDocumentUri`, not the URI of a synthetic SQL document.
- Relative `configuration_file_path` is resolved from the workspace root associated with the host document.
- Client-provided formatter overrides follow the same merge behavior as ordinary document formatting.

### Client responsibilities

- Collect the exact selected text and send it in `selections[].text`.
- Avoid sending overlapping selections.
- Avoid applying returned edits if the host document version has changed since the request was sent.
- Treat the request as a formatting operation on independent SQL fragments. The server does not infer surrounding language context.
