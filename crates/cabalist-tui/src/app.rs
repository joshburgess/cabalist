//! Application state and core logic.

use crate::theme::Theme;
use crate::views::View;
use cabalist_opinions::config::CabalistConfig;
use cabalist_opinions::lints::Lint;
use cabalist_parser::ast::{self, CabalFile};
use cabalist_parser::edit::{self, EditBatch};
use cabalist_parser::ParseResult;
use std::path::PathBuf;
use std::time::Instant;

/// Central application state.
pub struct App {
    /// Path to the .cabal file on disk.
    pub cabal_path: PathBuf,
    /// The raw source text (owned). We keep this so we can re-parse.
    pub source: String,
    /// The current parse result (CST + parse diagnostics).
    pub parse_result: ParseResult,
    /// Opinionated lints computed from the AST.
    pub lints: Vec<Lint>,
    /// User/project configuration.
    pub config: CabalistConfig,
    /// Active theme.
    pub theme: Theme,
    /// Current view.
    pub current_view: View,
    /// Whether the in-memory CST differs from the file on disk.
    pub dirty: bool,
    /// Whether the app should exit the event loop.
    pub should_quit: bool,
    /// A transient status message and the time it was set.
    pub status_message: Option<(String, Instant)>,
    /// Search query text (shared across search popups).
    pub search_query: String,
    /// Whether the search popup is currently shown.
    pub search_active: bool,
    /// Selected index in the current list view.
    pub selected_index: usize,
    /// Selected component tab index (deps/extensions views).
    pub selected_component: usize,
    /// Lines of build output (for the build view).
    pub build_output: Vec<String>,
    /// Whether a build subprocess is currently running.
    pub build_running: bool,
}

impl App {
    /// Create a new `App` by loading and parsing the given `.cabal` file.
    pub fn new(cabal_path: PathBuf, theme: Theme) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(&cabal_path)?;
        let parse_result = cabalist_parser::parse(&source);

        // Load config from the project root.
        let project_root = cabal_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let config = cabalist_opinions::config::find_and_load_config(project_root);

        let mut app = Self {
            cabal_path,
            source,
            parse_result,
            lints: Vec::new(),
            config,
            theme,
            current_view: View::Dashboard,
            dirty: false,
            should_quit: false,
            status_message: None,
            search_query: String::new(),
            search_active: false,
            selected_index: 0,
            selected_component: 0,
            build_output: Vec::new(),
            build_running: false,
        };

        app.refresh_lints();
        Ok(app)
    }

    /// Derive the AST from the current CST. The returned value borrows from
    /// `self.parse_result.cst` (and transitively from `self.source`).
    pub fn ast(&self) -> CabalFile<'_> {
        ast::derive_ast(&self.parse_result.cst)
    }

    /// Reload the `.cabal` file from disk and re-derive everything.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.source = std::fs::read_to_string(&self.cabal_path)?;
        self.parse_result = cabalist_parser::parse(&self.source);
        self.refresh_lints();
        self.dirty = false;
        self.set_status("Reloaded from disk");
        Ok(())
    }

    /// Write the current CST back to disk.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let rendered = self.parse_result.cst.render();
        std::fs::write(&self.cabal_path, &rendered)?;
        self.dirty = false;
        self.set_status("Saved");
        Ok(())
    }

    /// Re-run the opinionated lints on the current AST.
    pub fn refresh_lints(&mut self) {
        let ast = self.ast();
        let lint_config = self.config.lints.to_lint_config();
        self.lints = cabalist_opinions::lints::run_lints(&ast, &lint_config);
    }

    /// Set a transient status message.
    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some((msg.to_string(), Instant::now()));
    }

    /// Total number of items in the current list (for bounds checking).
    pub fn current_list_len(&self) -> usize {
        match self.current_view {
            View::Metadata => 13, // number of metadata fields
            View::Dependencies => {
                let ast = self.ast();
                count_deps_for_component(&ast, self.selected_component)
            }
            View::Extensions => self.extensions_list_len(),
            _ => 0,
        }
    }

    /// Get the component name string (as used by edit::find_section) for the
    /// currently selected component index.
    fn selected_component_spec(&self) -> Option<(&str, Option<&str>)> {
        let ast = self.ast();
        let mut ci = 0usize;
        if ast.library.is_some() {
            if ci == self.selected_component {
                return Some(("library", None));
            }
            ci += 1;
        }
        for exe in &ast.executables {
            if ci == self.selected_component {
                return Some(("executable", exe.fields.name));
            }
            ci += 1;
        }
        for ts in &ast.test_suites {
            if ci == self.selected_component {
                return Some(("test-suite", ts.fields.name));
            }
            ci += 1;
        }
        for bm in &ast.benchmarks {
            if ci == self.selected_component {
                return Some(("benchmark", bm.fields.name));
            }
            ci += 1;
        }
        None
    }

    /// Apply edits to the source, re-parse, and update state.
    fn apply_edits(&mut self, edits: Vec<edit::TextEdit>) {
        if edits.is_empty() {
            return;
        }
        let mut batch = EditBatch::new();
        batch.add_all(edits);
        self.source = batch.apply(&self.source);
        self.parse_result = cabalist_parser::parse(&self.source);
        self.refresh_lints();
        self.dirty = true;
    }

    /// Add a dependency to the currently selected component's build-depends.
    pub fn add_dependency(&mut self, dep_str: &str) -> Result<(), String> {
        let (keyword, name) = self
            .selected_component_spec()
            .ok_or_else(|| "No component selected".to_string())?;
        // We need owned copies because self is borrowed by selected_component_spec
        let keyword = keyword.to_string();
        let name = name.map(|n| n.to_string());

        let cst = &self.parse_result.cst;
        let section_id = edit::find_section(cst, &keyword, name.as_deref())
            .ok_or_else(|| format!("Component '{keyword}' not found"))?;

        let field_id = edit::find_field(cst, section_id, "build-depends");

        let edits = match field_id {
            Some(fid) => {
                // Check if already present.
                let item_name = dep_str.split_whitespace().next().unwrap_or(dep_str);
                let ast = self.ast();
                let already = ast
                    .all_dependencies()
                    .iter()
                    .any(|d| d.package.eq_ignore_ascii_case(item_name));
                if already {
                    return Err(format!("'{item_name}' is already in build-depends"));
                }
                edit::add_list_item(cst, fid, dep_str, true)
            }
            None => {
                // No build-depends field exists; add one.
                vec![edit::add_field_to_section(
                    cst,
                    section_id,
                    "build-depends",
                    dep_str,
                )]
            }
        };

        self.apply_edits(edits);
        Ok(())
    }

    /// Remove a dependency from the currently selected component's build-depends.
    pub fn remove_dependency(&mut self, package: &str) -> Result<(), String> {
        let (keyword, name) = self
            .selected_component_spec()
            .ok_or_else(|| "No component selected".to_string())?;
        let keyword = keyword.to_string();
        let name = name.map(|n| n.to_string());

        let cst = &self.parse_result.cst;
        let section_id = edit::find_section(cst, &keyword, name.as_deref())
            .ok_or_else(|| format!("Component '{keyword}' not found"))?;

        let field_id = edit::find_field(cst, section_id, "build-depends")
            .ok_or_else(|| "No build-depends field found".to_string())?;

        let edits = edit::remove_list_item(cst, field_id, package);
        if edits.is_empty() {
            return Err(format!("'{package}' not found in build-depends"));
        }

        self.apply_edits(edits);
        Ok(())
    }

    /// Toggle an extension in the library's default-extensions field.
    pub fn toggle_extension(&mut self, ext_name: &str) -> Result<(), String> {
        // For extensions, we operate on the library (or first component).
        let cst = &self.parse_result.cst;

        // Determine which section to modify. If there's a library, use it.
        let section_id = edit::find_section(cst, "library", None)
            .ok_or_else(|| "No library component found".to_string())?;

        let ast = self.ast();
        let is_enabled = ast
            .library
            .as_ref()
            .map(|lib| {
                lib.fields
                    .default_extensions
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(ext_name))
            })
            .unwrap_or(false);
        // Drop the AST borrow before we mutate.
        drop(ast);

        let cst = &self.parse_result.cst;
        let field_id = edit::find_field(cst, section_id, "default-extensions");

        if is_enabled {
            // Remove the extension.
            let fid = field_id.ok_or_else(|| "No default-extensions field".to_string())?;
            let edits = edit::remove_list_item(cst, fid, ext_name);
            if edits.is_empty() {
                return Err(format!("Could not remove '{ext_name}'"));
            }
            self.apply_edits(edits);
        } else {
            // Add the extension.
            let edits = match field_id {
                Some(fid) => edit::add_list_item(cst, fid, ext_name, true),
                None => {
                    vec![edit::add_field_to_section(
                        cst,
                        section_id,
                        "default-extensions",
                        ext_name,
                    )]
                }
            };
            self.apply_edits(edits);
        }

        Ok(())
    }

    /// Get the name of the dependency at the given index in the current component.
    pub fn dep_name_at_index(&self, idx: usize) -> Option<String> {
        let ast = self.ast();
        let deps = deps_for_component_ast(&ast, self.selected_component);
        deps.get(idx).map(|d| d.to_string())
    }

    /// Get the extension name at the given index in the extensions list view.
    /// Returns both the name and whether it is currently enabled.
    pub fn extension_at_index(&self, idx: usize) -> Option<(String, bool)> {
        let ast = self.ast();
        let enabled: Vec<&str> = ast
            .library
            .as_ref()
            .map(|lib| lib.fields.default_extensions.clone())
            .unwrap_or_default();

        // The extensions view lists enabled first, then available.
        if idx < enabled.len() {
            return Some((enabled[idx].to_string(), true));
        }

        let remaining_idx = idx - enabled.len();
        let all_ext = cabalist_ghc::extensions::load_extensions();
        let enabled_set: std::collections::HashSet<&str> = enabled.iter().copied().collect();

        let mut count = 0usize;
        for ext in all_ext {
            if enabled_set.contains(ext.name.as_str()) {
                continue;
            }
            if count == remaining_idx {
                return Some((ext.name.clone(), false));
            }
            count += 1;
        }
        None
    }

    /// Return the total number of items in the extensions list.
    pub fn extensions_list_len(&self) -> usize {
        let ast = self.ast();
        let enabled_count = ast
            .library
            .as_ref()
            .map(|lib| lib.fields.default_extensions.len())
            .unwrap_or(0);
        let all_ext = cabalist_ghc::extensions::load_extensions();
        let available_count = all_ext.len().saturating_sub(enabled_count);
        enabled_count + available_count
    }
}

/// Get dependency package names for a component by index.
fn deps_for_component_ast<'a>(ast: &CabalFile<'a>, idx: usize) -> Vec<&'a str> {
    let mut ci = 0usize;
    if let Some(ref lib) = ast.library {
        if ci == idx {
            return lib.fields.build_depends.iter().map(|d| d.package).collect();
        }
        ci += 1;
    }
    for exe in &ast.executables {
        if ci == idx {
            return exe.fields.build_depends.iter().map(|d| d.package).collect();
        }
        ci += 1;
    }
    for ts in &ast.test_suites {
        if ci == idx {
            return ts.fields.build_depends.iter().map(|d| d.package).collect();
        }
        ci += 1;
    }
    for bm in &ast.benchmarks {
        if ci == idx {
            return bm.fields.build_depends.iter().map(|d| d.package).collect();
        }
        ci += 1;
    }
    Vec::new()
}

/// Count dependencies in a component by index.
fn count_deps_for_component(ast: &CabalFile<'_>, idx: usize) -> usize {
    let mut ci = 0usize;
    if let Some(ref lib) = ast.library {
        if ci == idx {
            return lib.fields.build_depends.len();
        }
        ci += 1;
    }
    for exe in &ast.executables {
        if ci == idx {
            return exe.fields.build_depends.len();
        }
        ci += 1;
    }
    for ts in &ast.test_suites {
        if ci == idx {
            return ts.fields.build_depends.len();
        }
        ci += 1;
    }
    for bm in &ast.benchmarks {
        if ci == idx {
            return bm.fields.build_depends.len();
        }
        ci += 1;
    }
    0
}
