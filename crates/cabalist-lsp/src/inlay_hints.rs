//! Inlay hint provider for `.cabal` files.
//!
//! Shows the latest Hackage version inline next to each dependency in
//! `build-depends`, making it easy to spot outdated constraints without hovering.

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;

/// Compute inlay hints for the visible range of a `.cabal` file.
pub fn inlay_hints(
    source: &str,
    line_index: &LineIndex,
    range: &Range,
    hackage: Option<&cabalist_hackage::HackageIndex>,
) -> Vec<InlayHint> {
    let Some(index) = hackage else {
        return Vec::new();
    };

    let result = cabalist_parser::parse(source);
    let ast = cabalist_parser::ast::derive_ast(&result.cst);
    let mut hints = Vec::new();

    for dep in ast.all_dependencies() {
        let node = &result.cst.nodes[dep.cst_node.0];
        let dep_range = line_index.span_to_range(node.span);

        // Only emit hints for dependencies within the requested range.
        if dep_range.end.line < range.start.line || dep_range.start.line > range.end.line {
            continue;
        }

        let Some(info) = index.package_info(dep.package) else {
            continue;
        };
        let Some(latest) = info.latest_version() else {
            continue;
        };

        // Check if the current constraint would accept the latest version.
        let is_outdated = match &dep.version_range {
            Some(vr) => {
                let parser_version = cabalist_parser::ast::Version {
                    components: latest.components.clone(),
                };
                !cabalist_parser::ast::version_satisfies(&parser_version, vr)
            }
            None => false,
        };

        let label = if is_outdated {
            format!(" latest: {} ", latest)
        } else {
            format!(" {} ", latest)
        };

        let tooltip = if info.deprecated {
            Some(InlayHintTooltip::String(format!(
                "{} (deprecated on Hackage)",
                info.synopsis
            )))
        } else if !info.synopsis.is_empty() {
            Some(InlayHintTooltip::String(info.synopsis.clone()))
        } else {
            None
        };

        hints.push(InlayHint {
            position: dep_range.end,
            label: InlayHintLabel::String(label),
            kind: Some(InlayHintKind::PARAMETER),
            text_edits: None,
            tooltip,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        });
    }

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_hints_without_hackage() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n\nlibrary\n  build-depends: base ^>=4.17\n";
        let line_index = LineIndex::new(source);
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 0,
            },
        };
        let hints = inlay_hints(source, &line_index, &range, None);
        assert!(hints.is_empty());
    }

    #[test]
    fn hints_with_hackage_index() {
        use cabalist_hackage::{HackageIndex, PackageInfo, Version};

        let packages = vec![PackageInfo {
            name: "base".to_string(),
            synopsis: "Basic libraries".to_string(),
            versions: vec![
                Version::parse("4.17.0.0").unwrap(),
                Version::parse("4.19.1.0").unwrap(),
            ],
            deprecated: false,
        }];
        let index = HackageIndex::from_packages(packages);

        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n\nlibrary\n  build-depends: base ^>=4.17\n";
        let line_index = LineIndex::new(source);
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 0,
            },
        };
        let hints = inlay_hints(source, &line_index, &range, Some(&index));
        assert!(
            !hints.is_empty(),
            "should produce hints when Hackage data is available"
        );
    }
}
