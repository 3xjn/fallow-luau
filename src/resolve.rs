use std::path::{Path, PathBuf};

/// Outcome of resolving a string-literal `require`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedRequire {
    /// Absolute path of an existing module file.
    Found(PathBuf),
    /// Specifier could not be mapped to an existing file.
    Missing { specifier: String },
}

/// Resolve a string-literal require relative to the requiring file.
///
/// Core resolvers understand path-like string literals only. Project-specific
/// import wrappers and Rojo path aliases are optional plugins (not implemented here).
pub fn resolve_require(from_file: &Path, specifier: &str) -> ResolvedRequire {
    // Luau treats init.luau as its directory's module. Relative imports are
    // siblings of that module; @self imports are its children.
    // https://rfcs.luau.org/abstract-module-paths-and-init-dot-luau.html
    let parent = from_file.parent().unwrap_or_else(|| Path::new("."));
    let is_init = matches!(
        from_file.file_name().and_then(|s| s.to_str()),
        Some("init.luau" | "init.lua")
    );
    let module = if is_init {
        parent.to_path_buf()
    } else {
        from_file.with_extension("")
    };
    let base = module.parent().unwrap_or_else(|| Path::new("."));
    let candidates = if let Some(child) = specifier.strip_prefix("@self/") {
        candidate_paths(&module, child)
    } else {
        candidate_paths(base, specifier)
    };
    for candidate in candidates {
        if candidate.is_file() {
            return ResolvedRequire::Found(canonicalize_best_effort(&candidate));
        }
    }
    ResolvedRequire::Missing {
        specifier: specifier.to_string(),
    }
}

fn candidate_paths(base: &Path, specifier: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let raw = PathBuf::from(specifier);

    // Relative-to-requirer first (./ and ../ and bare relative).
    let joined = if specifier.starts_with('/') {
        PathBuf::from(specifier)
    } else {
        base.join(&raw)
    };

    push_module_variants(&mut out, &joined);

    // Also try treating bare names as sibling modules (Luau/Rojo-ish convenience).
    if !specifier.contains('/') && !specifier.contains('\\') {
        push_module_variants(&mut out, &base.join(specifier));
    }

    out
}

fn push_module_variants(out: &mut Vec<PathBuf>, path: &Path) {
    let s = path.to_string_lossy();
    if s.ends_with(".lua") || s.ends_with(".luau") {
        out.push(path.to_path_buf());
        return;
    }
    // Append the suffix: dotted module names (e.g. FlagCache.spec) are valid.
    let mut luau = path.as_os_str().to_os_string();
    luau.push(".luau");
    out.push(PathBuf::from(luau));
    let mut lua = path.as_os_str().to_os_string();
    lua.push(".lua");
    out.push(PathBuf::from(lua));
    out.push(path.join("init.luau"));
    out.push(path.join("init.lua"));
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolves_relative_luau() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.luau");
        let b = dir.path().join("b.luau");
        fs::write(&a, "return require(\"./b\")").unwrap();
        fs::write(&b, "return {}").unwrap();
        match resolve_require(&a, "./b") {
            ResolvedRequire::Found(p) => assert_eq!(p, b.canonicalize().unwrap()),
            other => panic!("expected found, got {other:?}"),
        }
    }
}
