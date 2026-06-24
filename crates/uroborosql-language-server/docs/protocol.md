# Protocol Details

This document describes integration details for `uroborosql-language-server`.

## Custom Requests

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

#### Field Semantics

- `hostDocumentUri`
  - Required.
  - URI of the host document that owns the selections.
  - The language server resolves formatter configuration using this URI as the configuration scope.
- `hostDocumentVersion`
  - Required.
  - Version of the host document when the client collected the selections.
  - The server echoes this value in the response so clients can detect stale results before
    applying edits.
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

#### Response Semantics

- `hostDocumentVersion`
  - Echo of the request field.
  - Clients should compare this value with the current document version before applying edits.
- `edits`
  - Replacement edits in the same order as the request `selections`.
  - Each `range` matches the corresponding input selection range.
  - Each `newText` contains the formatted SQL fragment for that selection.

### Error Handling

- `selections` must not be empty. An empty list is rejected as invalid params.
- The request is intentionally all-or-nothing.
  - If formatting any single selection fails, the server returns an error for the whole request.
  - The server does not return partial success.
  - This allows clients to avoid reconciling partially formatted multi-selection edits.

## Configuration Resolution

Formatter configuration:

- resolved using `hostDocumentUri`, not the URI of a synthetic SQL document
- can include client-side formatter overrides
- resolves relative `configuration_file_path` from the workspace root associated with the host
  document

Lint configuration:

- resolved per workspace root
- disabled when no lint config file can be resolved
- uses `.uroborosqllintrc.json` by default
- can be overridden by client configuration such as `lintConfigurationFilePath`

## Multi-Root Workspace Behavior

The server resolves configuration per workspace root rather than using a single global root.

This allows:

- different formatter settings per workspace folder
- different lint config files per workspace folder
- reloading lint configuration when workspace folders change

## Diagnostics And Code Actions

Diagnostics:

- published only when lint configuration is available
- refreshed on open, save, workspace folder changes, workspace configuration changes, and watched config changes
- not refreshed on every `didChange`

Code actions:

- `quickfix` only
- add `uroborosql-lint-disable-next-line` directives for lint diagnostics
- remove unknown rule names from existing lint directives

The server does not currently provide general-purpose autofixes for lint violations.

## Client Responsibilities

- collect the exact selected text and send it in `selections[].text`
- avoid sending overlapping selections
- avoid applying returned edits if the host document version has changed since the request was sent
- treat the custom request as formatting independent SQL fragments rather than language-aware host
  code transformations
