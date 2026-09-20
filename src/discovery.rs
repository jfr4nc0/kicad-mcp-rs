use serde::Serialize;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KicadStatus {
    pub connected: bool,
    pub socket_path: Option<PathBuf>,
    pub socket_source: &'static str,
    pub socket_exists: bool,
    pub api_token_present: bool,
    pub write_enabled: bool,
    pub note: &'static str,
}

pub fn discover() -> KicadStatus {
    let explicit = std::env::var_os("KICAD_API_SOCKET");
    let (socket_path, socket_source) = resolve_socket(explicit.as_deref(), &std::env::temp_dir());
    let socket_exists = socket_path.as_ref().is_some_and(|path| path.exists());

    KicadStatus {
        connected: false,
        socket_path,
        socket_source,
        socket_exists,
        api_token_present: std::env::var_os("KICAD_API_TOKEN").is_some(),
        write_enabled: matches!(
            std::env::var("KICAD_MCP_ALLOW_WRITE").as_deref(),
            Ok("1" | "true" | "yes")
        ),
        note: "Transport handshake is planned for milestone M1; no KiCad mutation is performed yet.",
    }
}

fn resolve_socket(explicit: Option<&OsStr>, temp_dir: &Path) -> (Option<PathBuf>, &'static str) {
    if let Some(path) = explicit {
        return (Some(PathBuf::from(path)), "KICAD_API_SOCKET");
    }

    let default = temp_dir.join("api.sock");
    if default.exists() {
        return (Some(default), "temporary directory default");
    }

    let mut candidates: Vec<PathBuf> = fs::read_dir(temp_dir)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    #[test]
    fn explicit_socket_wins() {
        let temp = std::env::temp_dir();
        let (path, source) = resolve_socket(Some(OsStr::new("/tmp/custom.sock")), &temp);
        assert_eq!(path, Some(PathBuf::from("/tmp/custom.sock")));
        assert_eq!(source, "KICAD_API_SOCKET");
    }

    #[test]
    fn discovers_default_socket() {
        let root = test_dir("default");
        fs::create_dir_all(&root).unwrap();
        File::create(root.join("api.sock")).unwrap();

        let (path, source) = resolve_socket(None, &root);
        assert_eq!(path, Some(root.join("api.sock")));
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
