use std::path::PathBuf;

use tower_lsp_server::{UriExt, lsp_types::Uri};

use crate::Backend;

/// Check if a URI is a file URI.
pub fn is_file_uri(uri: &Uri) -> bool {
    matches!(uri.scheme(), Some(scheme) if scheme.as_str().eq_ignore_ascii_case("file"))
}

/// Convert a file URI to a filesystem path.
pub fn file_uri_to_path(uri: &Uri) -> Option<PathBuf> {
    if !is_file_uri(uri) {
        return None;
    }
    uri.to_file_path().map(|path| path.to_path_buf())
}

/// Resolve the workspace root directory from an optional LSP URI.
pub fn root_dir_from_uri(root_uri: Option<&Uri>) -> Option<PathBuf> {
    root_uri.and_then(file_uri_to_path)
}

impl Backend {
    pub(crate) fn root_dir(&self) -> Option<PathBuf> {
        root_dir_from_uri(self.root_uri.read().unwrap().as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::str::FromStr;

    #[test]
    fn non_file_uri_is_none() {
        let uri = Uri::from_str("untitled:Untitled-1").expect("valid uri");
        assert!(!is_file_uri(&uri));
        assert_eq!(file_uri_to_path(&uri), None);
    }

    #[test]
    fn root_dir_none_is_none() {
        assert_eq!(root_dir_from_uri(None), None);
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
    fn root_dir_from_uri_resolves() {
        let root = env::temp_dir().join("uroborosql project");
        let uri = Uri::from_file_path(&root).expect("path to uri");
        assert_eq!(root_dir_from_uri(Some(&uri)), Some(root));
    }
}
