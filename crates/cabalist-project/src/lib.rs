//! # cabalist-project
//!
//! Parser for `cabal.project` files (the multi-package project configuration
//! format used by cabal-install). This is a simpler format than `.cabal` files
//! and is handled by a dedicated, lightweight parser.
//!
//! # Usage
//!
//! ```rust
//! use cabalist_project::{parse, CabalProject};
//!
//! let source = "packages: ./*.cabal\nwith-compiler: ghc-9.8.2\n";
//! let project = parse(source);
//! assert_eq!(project.packages, vec!["./*.cabal"]);
//! assert_eq!(project.with_compiler.as_deref(), Some("ghc-9.8.2"));
//! ```

pub mod parse;
pub mod types;

pub use parse::parse;
pub use types::{CabalProject, PackageStanza, SourceRepoPackage};

/// Parse a `cabal.project` file from a filesystem path.
///
/// # Errors
///
/// Returns an `io::Error` if the file cannot be read.
pub fn parse_file(path: &std::path::Path) -> Result<CabalProject, std::io::Error> {
    let source = std::fs::read_to_string(path)?;
    Ok(parse(&source))
}

/// Find a `cabal.project` file in the given directory.
///
/// Checks for `cabal.project` first, then `cabal.project.local`.
/// Returns the path to the first file found, or `None` if neither exists.
pub fn find_project_file(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let candidates = ["cabal.project", "cabal.project.local"];
    for name in &candidates {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_file_works() {
        let dir = std::env::temp_dir().join("cabalist-project-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cabal.project");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "packages: ./*.cabal").unwrap();
        drop(f);

        let proj = parse_file(&path).unwrap();
        assert_eq!(proj.packages, vec!["./*.cabal"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_project_file_found() {
        let dir = std::env::temp_dir().join("cabalist-find-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cabal.project");
        std::fs::write(&path, "packages: .\n").unwrap();

        let found = find_project_file(&dir);
        assert_eq!(found, Some(path));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_project_file_local() {
        let dir = std::env::temp_dir().join("cabalist-find-local-test");
        std::fs::create_dir_all(&dir).unwrap();
        // No cabal.project, but cabal.project.local exists.
        let path = dir.join("cabal.project.local");
        std::fs::write(&path, "packages: .\n").unwrap();

        let found = find_project_file(&dir);
        assert_eq!(found, Some(path));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_project_file_not_found() {
        let dir = std::env::temp_dir().join("cabalist-find-none-test");
        std::fs::create_dir_all(&dir).unwrap();
        // Clean up any leftover files.
        std::fs::remove_file(dir.join("cabal.project")).ok();
        std::fs::remove_file(dir.join("cabal.project.local")).ok();

        let found = find_project_file(&dir);
        assert_eq!(found, None);

        std::fs::remove_dir_all(&dir).ok();
    }
}
