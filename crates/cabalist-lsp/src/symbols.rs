//! Document symbol provider for `.cabal` files.
//!
//! Returns a hierarchical outline of sections and their fields, so editors
//! can render a navigable tree in the sidebar (Outline / Breadcrumbs).

use tower_lsp::lsp_types::*;

use crate::convert::LineIndex;

/// Compute document symbols for a `.cabal` source file.
pub fn document_symbols(source: &str, line_index: &LineIndex) -> Vec<DocumentSymbol> {
    let result = cabalist_parser::parse(source);
    let cst = &result.cst;
    let ast = cabalist_parser::ast::derive_ast(cst);

    let mut symbols = Vec::new();

    // Top-level metadata fields.
    let mut meta_children = Vec::new();
    for field_name in &[
        "cabal-version",
        "name",
        "version",
        "synopsis",
        "license",
        "author",
        "maintainer",
        "homepage",
        "bug-reports",
        "category",
        "build-type",
    ] {
        if let Some(field_id) = cabalist_parser::edit::find_field(cst, cst.root, field_name) {
            let node = &cst.nodes[field_id.0];
            let range = line_index.span_to_range(node.span);
            let detail = node
                .field_value
                .map(|v| source[v.start..v.end].trim().to_string());

            #[allow(deprecated)]
            meta_children.push(DocumentSymbol {
                name: field_name.to_string(),
                detail,
                kind: SymbolKind::PROPERTY,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            });
        }
    }

    if !meta_children.is_empty() {
        let first_range = meta_children.first().map(|s| s.range).unwrap_or_default();
        let last_range = meta_children.last().map(|s| s.range).unwrap_or_default();
        let full_range = Range {
            start: first_range.start,
            end: last_range.end,
        };

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: "Package Metadata".to_string(),
            detail: ast.name.map(|n| n.to_string()),
            kind: SymbolKind::PACKAGE,
            tags: None,
            deprecated: None,
            range: full_range,
            selection_range: first_range,
            children: Some(meta_children),
        });
    }

    // Components.
    for comp in ast.all_components() {
        let fields = comp.fields();
        let node = &cst.nodes[fields.cst_node.0];
        let range = line_index.span_to_range(node.span);

        let (kind, name) = match comp {
            cabalist_parser::ast::Component::Library(lib) => {
                let n = lib.fields.name.map(|s| format!("library {s}")).unwrap_or_else(|| "library".to_string());
                (SymbolKind::MODULE, n)
            }
            cabalist_parser::ast::Component::Executable(exe) => {
                let n = format!("executable {}", exe.fields.name.unwrap_or("(unnamed)"));
                (SymbolKind::FUNCTION, n)
            }
            cabalist_parser::ast::Component::TestSuite(ts) => {
                let n = format!("test-suite {}", ts.fields.name.unwrap_or("(unnamed)"));
                (SymbolKind::METHOD, n)
            }
            cabalist_parser::ast::Component::Benchmark(bm) => {
                let n = format!("benchmark {}", bm.fields.name.unwrap_or("(unnamed)"));
                (SymbolKind::EVENT, n)
            }
        };

        // Build children for key fields in this section.
        let mut children = Vec::new();
        let section_fields = &[
            "build-depends",
            "exposed-modules",
            "other-modules",
            "hs-source-dirs",
            "default-language",
            "default-extensions",
            "ghc-options",
            "main-is",
        ];
        for field_name in section_fields {
            if let Some(field_id) =
                cabalist_parser::edit::find_field(cst, fields.cst_node, field_name)
            {
                let fnode = &cst.nodes[field_id.0];
                let frange = line_index.span_to_range(fnode.span);

                #[allow(deprecated)]
                children.push(DocumentSymbol {
                    name: field_name.to_string(),
                    detail: None,
                    kind: SymbolKind::FIELD,
                    tags: None,
                    deprecated: None,
                    range: frange,
                    selection_range: frange,
                    children: None,
                });
            }
        }

        let dep_count = fields.build_depends.len();
        let detail = if dep_count > 0 {
            Some(format!("{dep_count} dependencies"))
        } else {
            None
        };

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name,
            detail,
            kind,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }

    // Common stanzas.
    for cs in &ast.common_stanzas {
        let node = &cst.nodes[cs.fields.cst_node.0];
        let range = line_index.span_to_range(node.span);

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: format!("common {}", cs.name),
            detail: None,
            kind: SymbolKind::OBJECT,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        });
    }

    // Flags.
    for flag in &ast.flags {
        let node = &cst.nodes[flag.cst_node.0];
        let range = line_index.span_to_range(node.span);

        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: format!("flag {}", flag.name),
            detail: flag.description.map(|d| d.to_string()),
            kind: SymbolKind::BOOLEAN,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        });
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_for_simple_cabal_file() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\nsynopsis: A test\n\nlibrary\n  exposed-modules: Lib\n  build-depends: base ^>=4.17\n";
        let line_index = LineIndex::new(source);
        let symbols = document_symbols(source, &line_index);

        // Should have Package Metadata + library
        assert!(symbols.len() >= 2);
        assert_eq!(symbols[0].name, "Package Metadata");
        assert!(symbols[0].children.is_some());
        assert_eq!(symbols[1].name, "library");
    }

    #[test]
    fn symbols_include_executables() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n\nexecutable my-app\n  main-is: Main.hs\n  build-depends: base\n";
        let line_index = LineIndex::new(source);
        let symbols = document_symbols(source, &line_index);

        let exe = symbols.iter().find(|s| s.name.contains("executable"));
        assert!(exe.is_some());
        assert_eq!(exe.unwrap().name, "executable my-app");
    }

    #[test]
    fn symbols_include_common_stanzas() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n\ncommon warnings\n  ghc-options: -Wall\n\nlibrary\n  import: warnings\n  exposed-modules: Lib\n";
        let line_index = LineIndex::new(source);
        let symbols = document_symbols(source, &line_index);

        let common = symbols.iter().find(|s| s.name.contains("common"));
        assert!(common.is_some());
    }

    #[test]
    fn symbols_include_flags() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n\nflag dev\n  description: Enable dev mode\n  default: False\n";
        let line_index = LineIndex::new(source);
        let symbols = document_symbols(source, &line_index);

        let flag = symbols.iter().find(|s| s.name.contains("flag"));
        assert!(flag.is_some());
        assert_eq!(flag.unwrap().name, "flag dev");
    }

    #[test]
    fn component_symbols_have_field_children() {
        let source = "cabal-version: 3.0\nname: test\nversion: 0.1\n\nlibrary\n  exposed-modules: Lib\n  build-depends: base ^>=4.17\n  default-language: GHC2021\n";
        let line_index = LineIndex::new(source);
        let symbols = document_symbols(source, &line_index);

        let lib = symbols.iter().find(|s| s.name == "library").unwrap();
        let children = lib.children.as_ref().unwrap();
        let field_names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        assert!(field_names.contains(&"build-depends"));
        assert!(field_names.contains(&"exposed-modules"));
        assert!(field_names.contains(&"default-language"));
    }
}
