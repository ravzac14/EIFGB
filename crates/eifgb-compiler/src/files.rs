//! Where source files come from.
//!
//! The online service compiles from uploaded files in memory and must
//! never touch the host filesystem on an author's behalf. So the compiler
//! only reads through [`FileProvider`], and every path it is given is
//! checked so it can't escape the project root.

use std::collections::BTreeMap;
use std::path::PathBuf;

pub trait FileProvider {
    /// Reads a file by its project-relative, `/`-separated path.
    fn read(&self, path: &str) -> Result<String, String>;

    /// Whether a file is there (pictures aren't text, so they aren't read).
    fn exists(&self, path: &str) -> bool {
        self.read(path).is_ok()
    }
}

/// Checks a project-relative path and normalizes it. Absolute paths,
/// `..` components and backslashes are refused.
pub fn normalize(path: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err(format!("'{path}' must be a relative path using '/'"));
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(format!("'{path}' must not contain '..'")),
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return Err(format!("'{path}' does not name a file"));
    }
    Ok(parts.join("/"))
}

/// Joins `rel` onto the directory that contains `base`.
pub fn sibling(base: &str, rel: &str) -> String {
    match base.rfind('/') {
        Some(i) => format!("{}/{}", &base[..i], rel),
        None => rel.to_string(),
    }
}

/// Files held in memory. This is what the service and the tests use.
#[derive(Debug, Default, Clone)]
pub struct MemFiles(pub BTreeMap<String, String>);

impl MemFiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, path: &str, contents: &str) -> Self {
        self.0.insert(path.to_string(), contents.to_string());
        self
    }
}

impl FileProvider for MemFiles {
    fn read(&self, path: &str) -> Result<String, String> {
        let p = normalize(path)?;
        self.0.get(&p).cloned().ok_or_else(|| format!("file not found: {p}"))
    }
}

/// Files under a directory on disk. This is what the CLI uses.
#[derive(Debug, Clone)]
pub struct DirFiles {
    pub root: PathBuf,
}

impl FileProvider for DirFiles {
    fn read(&self, path: &str) -> Result<String, String> {
        let p = normalize(path)?;
        let full = self.root.join(&p);
        std::fs::read_to_string(&full).map_err(|e| format!("{p}: {e}"))
    }

    fn exists(&self, path: &str) -> bool {
        normalize(path).is_ok_and(|p| self.root.join(p).is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rejects_escapes() {
        assert_eq!(normalize("scripts/./a.lua").unwrap(), "scripts/a.lua");
        assert!(normalize("../secret").is_err());
        assert!(normalize("a/../../b").is_err());
        assert!(normalize("/etc/passwd").is_err());
        assert!(normalize("C:/x").is_err());
        assert!(normalize("a\\b").is_err());
    }

    #[test]
    fn sibling_joins_relative_to_entry() {
        assert_eq!(sibling("game.yaml", "s.lua"), "s.lua");
        assert_eq!(sibling("games/x/game.yaml", "s.lua"), "games/x/s.lua");
    }
}
