use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Return true when `path` looks like Luau/Lua source.
pub fn is_luau_source(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("lua") | Some("luau") => true,
        _ => false,
    }
}

/// Test-path heuristic from the SPEC: `**/*spec*`, `**/*test*`, `tests/**`.
pub fn is_test_path(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if s.contains("/tests/") || s.starts_with("tests/") {
        return true;
    }
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    file.contains("spec") || file.contains("test")
}

/// Recursively discover `.lua` / `.luau` files under `root`, sorted for determinism.
pub fn discover_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !root.exists() {
        return files;
    }
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !entry.file_type().is_file() || !is_luau_source(path) {
            continue;
        }
        // Skip typical noise directories
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        if rel_s.split('/').any(|p| {
            matches!(
                p,
                "node_modules" | ".git" | "target" | "Packages" | "ServerPackages"
            )
        }) {
            continue;
        }
        files.push(path.to_path_buf());
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discovers_lua_and_luau() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.luau"), "return {}").unwrap();
        fs::write(dir.path().join("b.lua"), "return {}").unwrap();
        fs::write(dir.path().join("c.txt"), "nope").unwrap();
        let files = discover_files(dir.path());
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_path_heuristics() {
        assert!(is_test_path(Path::new("src/foo.spec.luau")));
        assert!(is_test_path(Path::new("tests/unit/bar.luau")));
        assert!(is_test_path(Path::new("FooTest.luau")));
        assert!(!is_test_path(Path::new("src/service.luau")));
    }
}
