//! Line-based parser for `cabal.project` files.

use crate::types::{CabalProject, PackageStanza, SourceRepoPackage};

/// Parse a `cabal.project` file from its source text.
///
/// This is a simple line-based parser that handles:
/// - Comments (`--`)
/// - Fields (`field-name: value`)
/// - Multi-line continuation via indentation
/// - `package <name>` stanzas
/// - `source-repository-package` stanzas
/// - `program-options` stanzas
pub fn parse(source: &str) -> CabalProject {
    let mut project = CabalProject {
        source: source.to_string(),
        ..Default::default()
    };

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let stripped = strip_comment(line);

        // Skip blank lines and comment-only lines.
        if stripped.trim().is_empty() {
            i += 1;
            continue;
        }

        let trimmed = stripped.trim();

        // Check for stanza headers (no leading whitespace on the keyword).
        if let Some(rest) = try_strip_prefix_ci(trimmed, "source-repository-package") {
            if rest.is_empty() && !is_continuation(stripped) {
                let (stanza, next) = parse_source_repo_stanza(&lines, i + 1);
                project.source_repo_packages.push(stanza);
                i = next;
                continue;
            }
        }

        if let Some(rest) = try_strip_prefix_ci(trimmed, "package") {
            // Must have a name after "package" and not be indented (not a continuation).
            let name = rest.trim();
            if !name.is_empty() && !is_continuation(stripped) {
                let (stanza, next) = parse_package_stanza(name, &lines, i + 1);
                project.package_stanzas.push(stanza);
                i = next;
                continue;
            }
        }

        if let Some(_rest) = try_strip_prefix_ci(trimmed, "program-options") {
            if !is_continuation(stripped) {
                // Parse program-options stanza fields as other_fields with a prefix.
                let (fields, next) = parse_stanza_fields(&lines, i + 1);
                for (k, v) in fields {
                    project
                        .other_fields
                        .push((format!("program-options.{k}"), v));
                }
                i = next;
                continue;
            }
        }

        // Top-level field.
        if let Some((name, value)) = parse_field_line(stripped) {
            let field_indent = indent_level(stripped);
            let (full_value, next) = collect_continuation(&lines, i + 1, value, field_indent);
            dispatch_field(&mut project, &name, &full_value);
            i = next;
            continue;
        }

        // Unrecognized line -- skip.
        i += 1;
    }

    project
}

/// Returns the line with any trailing comment removed.
/// Preserves leading whitespace so indentation detection still works.
fn strip_comment(line: &str) -> &str {
    // Find `--` that is not inside a URL (heuristic: preceded by `:` means likely a URL).
    // Simple approach: find first `--` that is preceded by whitespace or is at the start.
    if let Some(pos) = find_comment_start(line) {
        &line[..pos]
    } else {
        line
    }
}

/// Find the start position of a `--` comment, being careful not to strip
/// `--` inside URLs like `https://...`.
fn find_comment_start(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' {
            // Check if this looks like it's inside a URL (preceded by `:` or `/`).
            if i > 0 && (bytes[i - 1] == b':' || bytes[i - 1] == b'/') {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Check if a line is indented (continuation line).
fn is_continuation(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

/// Try to match a case-insensitive prefix, returning the remainder if matched.
fn try_strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let lower = s.to_ascii_lowercase();
    if lower.starts_with(prefix) {
        let rest = &s[prefix.len()..];
        // Must be followed by whitespace, colon, or end of string.
        if rest.is_empty()
            || rest.starts_with(' ')
            || rest.starts_with('\t')
            || rest.starts_with(':')
        {
            Some(rest)
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse a `field-name: value` line. Returns `(normalized-name, value)`.
fn parse_field_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let colon_pos = trimmed.find(':')?;
    let name_part = &trimmed[..colon_pos];

    // Field names consist of alphanumeric chars, hyphens, and underscores.
    if name_part.is_empty()
        || !name_part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    let name = normalize_field_name(name_part);
    let value = trimmed[colon_pos + 1..].trim().to_string();
    Some((name, value))
}

/// Normalize a field name to lowercase with hyphens.
fn normalize_field_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

/// Measure the indentation level of a line (number of leading spaces; tabs count as 8).
fn indent_level(line: &str) -> usize {
    let mut level = 0;
    for ch in line.chars() {
        match ch {
            ' ' => level += 1,
            '\t' => level = (level / 8 + 1) * 8,
            _ => break,
        }
    }
    level
}

/// Collect continuation lines (indented lines following a field).
/// A continuation line must be indented strictly more than `field_indent`.
/// Returns `(full_value, next_line_index)`.
fn collect_continuation(
    lines: &[&str],
    start: usize,
    initial: String,
    field_indent: usize,
) -> (String, usize) {
    let mut parts: Vec<String> = vec![initial];
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let stripped = strip_comment(line);
        let trimmed = stripped.trim();

        // A blank line ends continuation.
        if trimmed.is_empty() {
            break;
        }

        // A continuation line must be indented strictly more than the field.
        let line_indent = indent_level(stripped);
        if line_indent <= field_indent {
            break;
        }

        parts.push(trimmed.to_string());
        i += 1;
    }

    let value = parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (value, i)
}

/// Parse fields inside a stanza (indented block). Returns `(fields, next_line_index)`.
fn parse_stanza_fields(lines: &[&str], start: usize) -> (Vec<(String, String)>, usize) {
    let mut fields = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let stripped = strip_comment(line);

        // Stanza body lines must be indented.
        if !stripped.is_empty() && !is_continuation(stripped) {
            break;
        }

        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            // Blank line might end the stanza or just be spacing.
            // Peek ahead: if next non-blank line is indented, continue.
            if !has_indented_content_ahead(lines, i + 1) {
                i += 1;
                break;
            }
            i += 1;
            continue;
        }

        if let Some((name, value)) = parse_field_line(stripped) {
            let field_indent = indent_level(stripped);
            let (full_value, next) = collect_continuation(lines, i + 1, value, field_indent);
            fields.push((name, full_value));
            i = next;
        } else {
            i += 1;
        }
    }

    (fields, i)
}

/// Check if there are indented content lines ahead (skipping blanks).
fn has_indented_content_ahead(lines: &[&str], start: usize) -> bool {
    for &line in lines.iter().skip(start) {
        let stripped = strip_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }
        return is_continuation(stripped);
    }
    false
}

/// Parse a `package <name>` stanza.
fn parse_package_stanza(name: &str, lines: &[&str], start: usize) -> (PackageStanza, usize) {
    let (fields, next) = parse_stanza_fields(lines, start);
    let stanza = PackageStanza {
        name: name.to_string(),
        fields,
    };
    (stanza, next)
}

/// Parse a `source-repository-package` stanza.
fn parse_source_repo_stanza(lines: &[&str], start: usize) -> (SourceRepoPackage, usize) {
    let (fields, next) = parse_stanza_fields(lines, start);
    let mut repo = SourceRepoPackage::default();

    for (name, value) in &fields {
        match name.as_str() {
            "type" => repo.repo_type = Some(value.clone()),
            "location" => repo.location = Some(value.clone()),
            "tag" => repo.tag = Some(value.clone()),
            "branch" => repo.branch = Some(value.clone()),
            "subdir" => repo.subdir = Some(value.clone()),
            _ => {}
        }
    }

    (repo, next)
}

/// Split a comma-separated value string into individual items.
fn split_comma_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Split a whitespace-or-comma-separated value string into individual items.
/// Used for fields like `packages:` where items can be separated by whitespace.
fn split_whitespace_list(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Route a parsed top-level field to the appropriate slot on `CabalProject`.
fn dispatch_field(project: &mut CabalProject, name: &str, value: &str) {
    match name {
        "packages" => {
            project.packages.extend(split_whitespace_list(value));
        }
        "optional-packages" => {
            project
                .optional_packages
                .extend(split_whitespace_list(value));
        }
        "extra-packages" => {
            project.extra_packages.extend(split_comma_list(value));
        }
        "with-compiler" => {
            project.with_compiler = Some(value.to_string());
        }
        "index-state" => {
            project.index_state = Some(value.to_string());
        }
        "constraints" => {
            project.constraints.extend(split_comma_list(value));
        }
        "allow-newer" => {
            project.allow_newer.extend(split_comma_list(value));
        }
        "allow-older" => {
            project.allow_older.extend(split_comma_list(value));
        }
        _ => {
            project
                .other_fields
                .push((name.to_string(), value.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_minimal() {
        let src = "packages: ./*.cabal\n";
        let proj = parse(src);
        assert_eq!(proj.packages, vec!["./*.cabal"]);
    }

    #[test]
    fn parse_with_compiler() {
        let src = "\
packages: ./*.cabal
with-compiler: ghc-9.8.2
";
        let proj = parse(src);
        assert_eq!(proj.packages, vec!["./*.cabal"]);
        assert_eq!(proj.with_compiler.as_deref(), Some("ghc-9.8.2"));
    }

    #[test]
    fn parse_constraints() {
        let src = "\
packages: ./*.cabal
constraints: aeson ^>=2.2,
             text >=2.0
";
        let proj = parse(src);
        assert_eq!(proj.constraints, vec!["aeson ^>=2.2", "text >=2.0"]);
    }

    #[test]
    fn parse_package_stanza() {
        let src = "\
packages: ./*.cabal

package aeson
  flags: +ordered-keymap
  ghc-options: -O2
";
        let proj = parse(src);
        assert_eq!(proj.package_stanzas.len(), 1);
        assert_eq!(proj.package_stanzas[0].name, "aeson");
        assert_eq!(proj.package_stanzas[0].fields.len(), 2);
        assert_eq!(
            proj.package_stanzas[0].fields[0],
            ("flags".to_string(), "+ordered-keymap".to_string())
        );
        assert_eq!(
            proj.package_stanzas[0].fields[1],
            ("ghc-options".to_string(), "-O2".to_string())
        );
    }

    #[test]
    fn parse_package_star() {
        let src = "\
packages: ./*.cabal

package *
  optimization: 2
";
        let proj = parse(src);
        assert_eq!(proj.package_stanzas.len(), 1);
        assert_eq!(proj.package_stanzas[0].name, "*");
    }

    #[test]
    fn parse_source_repo_package() {
        let src = "\
packages: ./*.cabal

source-repository-package
  type: git
  location: https://github.com/user/repo
  tag: v1.0.0
  subdir: subdir-name
";
        let proj = parse(src);
        assert_eq!(proj.source_repo_packages.len(), 1);
        let repo = &proj.source_repo_packages[0];
        assert_eq!(repo.repo_type.as_deref(), Some("git"));
        assert_eq!(
            repo.location.as_deref(),
            Some("https://github.com/user/repo")
        );
        assert_eq!(repo.tag.as_deref(), Some("v1.0.0"));
        assert_eq!(repo.subdir.as_deref(), Some("subdir-name"));
        assert_eq!(repo.branch, None);
    }

    #[test]
    fn parse_allow_newer() {
        let src = "\
packages: ./*.cabal
allow-newer: base, template-haskell
";
        let proj = parse(src);
        assert_eq!(
            proj.allow_newer,
            vec!["base".to_string(), "template-haskell".to_string()]
        );
    }

    #[test]
    fn parse_comments_ignored() {
        let src = "\
-- This is a comment
packages: ./*.cabal
-- Another comment
with-compiler: ghc-9.8.2
";
        let proj = parse(src);
        assert_eq!(proj.packages, vec!["./*.cabal"]);
        assert_eq!(proj.with_compiler.as_deref(), Some("ghc-9.8.2"));
    }

    #[test]
    fn parse_multiline_continuation() {
        let src = "\
packages: ./*.cabal
           ../other-pkg/
";
        let proj = parse(src);
        assert_eq!(proj.packages, vec!["./*.cabal", "../other-pkg/"]);
    }

    #[test]
    fn parse_multiple_stanzas() {
        let src = "\
packages: ./*.cabal

package aeson
  flags: +ordered-keymap

package bytestring
  ghc-options: -O2

source-repository-package
  type: git
  location: https://github.com/user/repo
  tag: v1.0.0
";
        let proj = parse(src);
        assert_eq!(proj.package_stanzas.len(), 2);
        assert_eq!(proj.package_stanzas[0].name, "aeson");
        assert_eq!(proj.package_stanzas[1].name, "bytestring");
        assert_eq!(proj.source_repo_packages.len(), 1);
    }

    #[test]
    fn parse_empty_input() {
        let proj = parse("");
        assert!(proj.packages.is_empty());
        assert!(proj.with_compiler.is_none());
        assert!(proj.package_stanzas.is_empty());
        assert!(proj.source_repo_packages.is_empty());
    }

    #[test]
    fn parse_blank_lines_only() {
        let proj = parse("\n\n\n");
        assert!(proj.packages.is_empty());
    }

    #[test]
    fn parse_index_state() {
        let src = "index-state: 2024-01-15T00:00:00Z\n";
        let proj = parse(src);
        assert_eq!(proj.index_state.as_deref(), Some("2024-01-15T00:00:00Z"));
    }

    #[test]
    fn parse_other_fields() {
        let src = "\
packages: ./*.cabal
tests: true
benchmarks: false
";
        let proj = parse(src);
        assert_eq!(proj.other_fields.len(), 2);
        assert_eq!(
            proj.other_fields[0],
            ("tests".to_string(), "true".to_string())
        );
    }

    #[test]
    fn parse_full_example() {
        let src = "\
-- Global settings
packages: ./*.cabal
          ../other-pkg/

-- Solver settings
with-compiler: ghc-9.8.2
index-state: 2024-01-15T00:00:00Z

-- Global constraints
constraints: aeson ^>=2.2,
             text >=2.0

-- Package-specific settings
package aeson
  flags: +ordered-keymap
  ghc-options: -O2

package *
  optimization: 2

-- Source repository packages
source-repository-package
  type: git
  location: https://github.com/user/repo
  tag: v1.0.0
  subdir: subdir-name

-- Allow newer
allow-newer: base, template-haskell

-- Program locations
program-options
  ghc-options: -j4
";
        let proj = parse(src);
        assert_eq!(proj.packages, vec!["./*.cabal", "../other-pkg/"]);
        assert_eq!(proj.with_compiler.as_deref(), Some("ghc-9.8.2"));
        assert_eq!(proj.index_state.as_deref(), Some("2024-01-15T00:00:00Z"));
        assert_eq!(proj.constraints, vec!["aeson ^>=2.2", "text >=2.0"]);
        assert_eq!(proj.package_stanzas.len(), 2);
        assert_eq!(proj.source_repo_packages.len(), 1);
        assert_eq!(proj.allow_newer, vec!["base", "template-haskell"]);
        // program-options fields are stored as other_fields with prefix
        assert!(proj
            .other_fields
            .iter()
            .any(|(k, v)| k == "program-options.ghc-options" && v == "-j4"));
    }

    #[test]
    fn field_names_case_insensitive() {
        let src = "With-Compiler: ghc-9.8.2\n";
        let proj = parse(src);
        assert_eq!(proj.with_compiler.as_deref(), Some("ghc-9.8.2"));
    }

    #[test]
    fn field_names_underscore_normalized() {
        let src = "with_compiler: ghc-9.8.2\n";
        let proj = parse(src);
        assert_eq!(proj.with_compiler.as_deref(), Some("ghc-9.8.2"));
    }

    #[test]
    fn url_in_location_not_stripped() {
        let src = "\
source-repository-package
  type: git
  location: https://github.com/user/repo
";
        let proj = parse(src);
        assert_eq!(
            proj.source_repo_packages[0].location.as_deref(),
            Some("https://github.com/user/repo")
        );
    }

    #[test]
    fn parse_allow_older() {
        let src = "allow-older: base\n";
        let proj = parse(src);
        assert_eq!(proj.allow_older, vec!["base"]);
    }

    #[test]
    fn parse_optional_packages() {
        let src = "optional-packages: ./optional/*.cabal\n";
        let proj = parse(src);
        assert_eq!(proj.optional_packages, vec!["./optional/*.cabal"]);
    }

    #[test]
    fn parse_extra_packages() {
        let src = "extra-packages: some-pkg, another-pkg\n";
        let proj = parse(src);
        assert_eq!(proj.extra_packages, vec!["some-pkg", "another-pkg"]);
    }
}
