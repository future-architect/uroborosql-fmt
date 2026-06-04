use std::collections::{HashMap, HashSet};

use tower_lsp_server::lsp_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    TextEdit, Uri, WorkspaceEdit,
};
use uroborosql_lint::{INVALID_LINT_DIRECTIVE_CODE, LINT_SOURCE};

use crate::Backend;

mod directive_line;
mod disable_next_line;
mod remove_unknown_rule;

pub(crate) fn code_actions(
    backend: &Backend,
    params: CodeActionParams,
) -> Option<CodeActionResponse> {
    if !allows_quickfix(&params.context) {
        return Some(vec![]);
    }

    let uri = params.text_document.uri;
    let Some(rope) = backend.document_rope(&uri) else {
        return Some(vec![]);
    };

    let mut actions = Vec::new();
    let mut disable_next_line_seen = HashSet::new();

    for diagnostic in params.context.diagnostics {
        if let Some(action) = disable_next_line::disable_next_line_action(
            &uri,
            &rope,
            &diagnostic,
            &mut disable_next_line_seen,
        ) {
            actions.push(CodeActionOrCommand::CodeAction(action));
            continue;
        }

        if let Some(action) =
            remove_unknown_rule::remove_unknown_rule_action(&uri, &rope, &diagnostic)
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    Some(actions)
}

fn allows_quickfix(context: &CodeActionContext) -> bool {
    context.only.as_ref().is_none_or(|only| {
        only.iter()
            .any(|kind| kind.as_str() == CodeActionKind::QUICKFIX.as_str())
    })
}

fn workspace_edit(uri: &Uri, edit: TextEdit) -> WorkspaceEdit {
    WorkspaceEdit::new(HashMap::from([(uri.clone(), vec![edit])]))
}
