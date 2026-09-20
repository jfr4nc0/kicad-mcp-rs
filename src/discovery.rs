use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

pub fn discover_socket() -> (Option<PathBuf>, &'static str) {
    let explicit = std::env::var_os("KICAD_API_SOCKET");
    resolve_socket(explicit.as_deref(), &std::env::temp_dir())
}

fn resolve_socket(explicit: Option<&OsStr>, temp_dir: &Path) -> (Option<PathBuf>, &'static str) {
    if let Some(path) = explicit {
        return (
            Some(PathBuf::from(strip_ipc_prefix(path))),
            "KICAD_API_SOCKET",
        );
    }

    let socket_dir = temp_dir.join("kicad");
    let default = socket_dir.join("api.sock");
    if default.exists() {
        return (Some(default), "temporary directory default");
    }

    let mut candidates: Vec<PathBuf> = fs::read_dir(&socket_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with("api-") && name.ends_with(".sock"))
        })
        .collect();
    candidates.sort();

    match candidates.pop() {
        Some(path) => (Some(path), "temporary directory scan"),
        None => (None, "not found"),
    }
}

fn strip_ipc_prefix(path: &OsStr) -> OsString {
    match path.to_str().and_then(|value| value.strip_prefix("ipc://")) {
        Some(value) => OsString::from(value),
        None => path.to_os_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn explicit_socket_wins_and_strips_transport_prefix() {
        let temp = std::env::temp_dir();
        let (path, source) = resolve_socket(Some(OsStr::new("ipc:///tmp/custom.sock")), &temp);
        assert_eq!(path, Some(PathBuf::from("/tmp/custom.sock")));
        assert_eq!(source, "KICAD_API_SOCKET");
    }

    #[test]
    fn discovers_default_socket_in_kicad_subdirectory() {
        let root = test_dir("default");
        fs::create_dir_all(root.join("kicad")).unwrap();
        File::create(root.join("kicad/api.sock")).unwrap();

        let (path, source) = resolve_socket(None, &root);
        assert_eq!(path, Some(root.join("kicad/api.sock")));
        assert_eq!(source, "temporary directory default");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_missing_socket() {
        let root = test_dir("missing");
        fs::create_dir_all(&root).unwrap();

        let (path, source) = resolve_socket(None, &root);
        assert_eq!(path, None);
        assert_eq!(source, "not found");

        fs::remove_dir_all(root).unwrap();
    }

    fn test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kicad-mcp-{label}-{}", std::process::id()))
    }
}
