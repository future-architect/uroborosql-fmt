use std::path::{Component, Path, PathBuf};

use tower_lsp_server::{
    UriExt,
    lsp_types::{Uri, WorkspaceFolder},
};

use crate::Backend;

/// A workspace folder that resolves to a real file system path.
///
/// The LSP contract is expressed in URIs, but containment checks, map lookups
/// and lint store management are all done on `path`. Comparisons rely on
/// `Path`'s component semantics, which already ignore `.`, repeated separators
/// and trailing slashes, so no separate normalization step is needed. The `uri`
/// is retained for logging, notifications and client request scopes.
#[derive(Debug, Clone)]
pub(crate) struct WorkspaceRoot {
    pub(crate) uri: Uri,
    pub(crate) path: PathBuf,
}

impl WorkspaceRoot {
    pub(crate) fn from_uri(uri: &Uri) -> Option<Self> {
        Some(Self {
            uri: uri.clone(),
            path: file_uri_to_path(uri)?,
        })
    }
}

pub(crate) fn is_file_uri(uri: &Uri) -> bool {
    matches!(uri.scheme(), Some(scheme) if scheme.as_str().eq_ignore_ascii_case("file"))
}

pub(crate) fn file_uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if !is_file_uri(uri) {
        return None;
    }
    uri.to_file_path().map(|path| path.to_path_buf())
}

/// Returns whether `path` contains a `..` component.
///
/// `Path` comparison already ignores `.`, repeated separators and trailing
/// slashes, so containment needs no normalization for those. `..` is the only
/// component that could let a path masquerade as living under a sibling root
/// via prefix matching. Conformant clients send dot-segment-normalized URIs, so
/// this should never fire; callers surface it instead of guessing.
pub(crate) fn has_parent_dir_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

/// Resolves the workspace roots for a session.
///
/// `workspaceFolders` takes precedence; only when it yields no usable (file)
/// root do we fall back to the deprecated `rootUri` as a single pseudo
/// workspace. Non-`file` folders are dropped so they are never used as a
/// containment target.
pub(crate) fn resolve_workspace_roots(
    workspace_folders: Option<&[WorkspaceFolder]>,
    root_uri: Option<&Uri>,
) -> Vec<WorkspaceRoot> {
    let mut roots: Vec<WorkspaceRoot> = Vec::new();

    if let Some(folders) = workspace_folders {
        for folder in folders {
            push_unique_root(&mut roots, WorkspaceRoot::from_uri(&folder.uri));
        }
    }

    if roots.is_empty()
        && let Some(uri) = root_uri
    {
        push_unique_root(&mut roots, WorkspaceRoot::from_uri(uri));
    }

    roots
}

fn push_unique_root(roots: &mut Vec<WorkspaceRoot>, root: Option<WorkspaceRoot>) {
    if let Some(root) = root
        && !roots.iter().any(|existing| existing.path == root.path)
    {
        roots.push(root);
    }
}

/// Returns the workspace root that contains `path`, preferring the longest
/// (most specific) match when several roots are nested.
///
/// A `..` in `path` is refused: it could otherwise prefix-match a sibling root
/// and misattribute the document. Callers are expected to have screened it.
pub(crate) fn workspace_root_for_path<'a>(
    path: &Path,
    roots: &'a [WorkspaceRoot],
) -> Option<&'a WorkspaceRoot> {
    if has_parent_dir_component(path) {
        return None;
    }
    roots
        .iter()
        // `Path::starts_with` compares whole path components, so `/foo` is not
        // treated as a prefix of `/foobar`.
        .filter(|root| path.starts_with(&root.path))
        .max_by_key(|root| root.path.components().count())
}

impl Backend {
    pub(crate) fn workspace_root_for_uri(&self, uri: &Uri) -> Option<WorkspaceRoot> {
        let path = file_uri_to_path(uri)?;
        let roots = self.workspace_roots.read().unwrap();
        workspace_root_for_path(&path, &roots).cloned()
    }

    pub(crate) fn workspace_dir_for_uri(&self, uri: &Uri) -> Option<PathBuf> {
        self.workspace_root_for_uri(uri).map(|root| root.path)
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::str::FromStr;

    use super::*;

    fn root(path: &str) -> WorkspaceRoot {
        WorkspaceRoot {
            uri: Uri::from_str(&format!("file://{path}")).expect("valid uri"),
            path: PathBuf::from(path),
        }
    }

    #[test]
    fn non_file_uri_is_none() {
        let uri = Uri::from_str("untitled:Untitled-1").expect("valid uri");
        assert!(!is_file_uri(&uri));
        assert_eq!(file_uri_to_path(&uri), None);
        assert!(WorkspaceRoot::from_uri(&uri).is_none());
    }

    #[test]
    fn file_uri_roundtrip_with_spaces() {
        let path = env::temp_dir()
            .join("uroborosql path with space")
            .join("file.sql");
        let uri = Uri::from_file_path(&path).expect("path to uri");
        assert!(is_file_uri(&uri));
        assert_eq!(file_uri_to_path(&uri), Some(path));
    }

    #[test]
    fn workspace_root_for_path_ignores_dot_and_trailing_slash() {
        let roots = vec![root("/work/project/")];
        let selected =
            workspace_root_for_path(Path::new("/work/./project/src/main.sql"), &roots).unwrap();
        assert_eq!(selected.path, PathBuf::from("/work/project"));
    }

    #[test]
    fn workspace_root_for_path_refuses_parent_dir() {
        // `/work/project/../other/a.sql` really lives in `/work/other`, so it
        // must not prefix-match `/work/project`.
        let roots = vec![root("/work/project")];
        assert!(
            workspace_root_for_path(Path::new("/work/project/../other/a.sql"), &roots).is_none()
        );
    }

    #[test]
    fn workspace_root_for_path_picks_longest_prefix() {
        let roots = vec![root("/work"), root("/work/project")];
        let selected =
            workspace_root_for_path(Path::new("/work/project/src/main.sql"), &roots).unwrap();
        assert_eq!(selected.path, PathBuf::from("/work/project"));
    }

    #[test]
    fn workspace_root_for_path_ignores_partial_component_match() {
        // `/work/proj` must not be treated as a prefix of `/work/project-x`.
        let roots = vec![root("/work/proj")];
        assert!(workspace_root_for_path(Path::new("/work/project-x/a.sql"), &roots).is_none());
    }

    #[test]
    fn workspace_root_for_path_selects_owning_root_regardless_of_order() {
        let roots = vec![root("/work/other"), root("/work/project")];
        let selected =
            workspace_root_for_path(Path::new("/work/project/test.sql"), &roots).unwrap();
        assert_eq!(selected.path, PathBuf::from("/work/project"));
    }

    #[test]
    fn workspace_root_for_path_none_when_unowned() {
        let roots = vec![root("/work/project")];
        assert!(workspace_root_for_path(Path::new("/other/a.sql"), &roots).is_none());
    }

    #[test]
    fn resolve_workspace_roots_prefers_folders() {
        let folders = vec![
            WorkspaceFolder {
                uri: Uri::from_str("file:///work/other").expect("uri"),
                name: "other".into(),
            },
            WorkspaceFolder {
                uri: Uri::from_str("file:///work/project").expect("uri"),
                name: "project".into(),
            },
        ];
        let root_uri = Uri::from_str("file:///fallback").expect("uri");
        let roots = resolve_workspace_roots(Some(&folders), Some(&root_uri));
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].path, PathBuf::from("/work/other"));
        assert_eq!(roots[1].path, PathBuf::from("/work/project"));
    }

    #[test]
    fn resolve_workspace_roots_falls_back_to_root_uri() {
        let root_uri = Uri::from_str("file:///fallback").expect("uri");
        let roots = resolve_workspace_roots(None, Some(&root_uri));
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].path, PathBuf::from("/fallback"));
    }

    #[test]
    fn resolve_workspace_roots_falls_back_when_only_non_file_folders() {
        let folders = vec![WorkspaceFolder {
            uri: Uri::from_str("untitled:scratch").expect("uri"),
            name: "scratch".into(),
        }];
        let root_uri = Uri::from_str("file:///fallback").expect("uri");
        let roots = resolve_workspace_roots(Some(&folders), Some(&root_uri));
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].path, PathBuf::from("/fallback"));
    }

    #[test]
    fn resolve_workspace_roots_deduplicates() {
        let folders = vec![
            WorkspaceFolder {
                uri: Uri::from_str("file:///work/project").expect("uri"),
                name: "a".into(),
            },
            WorkspaceFolder {
                uri: Uri::from_str("file:///work/project/").expect("uri"),
                name: "b".into(),
            },
        ];
        let roots = resolve_workspace_roots(Some(&folders), None);
        assert_eq!(roots.len(), 1);
    }
}
