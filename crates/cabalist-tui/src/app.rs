//! Application state and core logic.

use crate::theme::Theme;
use crate::views::View;
use cabalist_opinions::config::CabalistConfig;
use cabalist_opinions::lints::Lint;
use cabalist_opinions::templates::TemplateKind;
use cabalist_parser::ast::{self, CabalFile};
use cabalist_parser::edit::{self, EditBatch};
use cabalist_parser::ParseResult;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::mpsc;

/// Events sent from an async build subprocess back to the TUI event loop.
#[derive(Debug)]
pub enum BuildEvent {
    /// A line of output (stdout or stderr) from the subprocess.
    Line(String),
    /// The subprocess completed.
    Complete {
        /// Whether the command succeeded (exit code 0).
        success: bool,
        /// Wall-clock duration of the command.
        duration: std::time::Duration,
    },
    /// The subprocess failed to start or encountered an error.
    Error(String),
}

/// Steps in the init wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitStep {
    /// Step 1: project name.
    Name,
    /// Step 2: project type / template.
    Template,
    /// Step 3: license.
    License,
    /// Step 4: author / maintainer.
    Author,
    /// Step 5: synopsis.
    Synopsis,
    /// Step 6: review and confirm.
    Confirm,
}

impl InitStep {
    /// The 1-based step number.
    pub fn number(&self) -> usize {
        match self {
            InitStep::Name => 1,
            InitStep::Template => 2,
            InitStep::License => 3,
            InitStep::Author => 4,
            InitStep::Synopsis => 5,
            InitStep::Confirm => 6,
        }
    }

    /// Advance to the next step, if any.
    pub fn next(&self) -> Option<InitStep> {
        match self {
            InitStep::Name => Some(InitStep::Template),
            InitStep::Template => Some(InitStep::License),
            InitStep::License => Some(InitStep::Author),
            InitStep::Author => Some(InitStep::Synopsis),
            InitStep::Synopsis => Some(InitStep::Confirm),
            InitStep::Confirm => None,
        }
    }

    /// Go back to the previous step, if any.
    pub fn prev(&self) -> Option<InitStep> {
        match self {
            InitStep::Name => None,
            InitStep::Template => Some(InitStep::Name),
            InitStep::License => Some(InitStep::Template),
            InitStep::Author => Some(InitStep::License),
            InitStep::Synopsis => Some(InitStep::Author),
            InitStep::Confirm => Some(InitStep::Synopsis),
        }
    }
}

/// State for the init wizard.
pub struct InitWizard {
    /// Current wizard step.
    pub step: InitStep,
    /// Project name.
    pub name: String,
    /// Selected template kind.
    pub template: TemplateKind,
    /// License identifier.
    pub license: String,
    /// Author name.
    pub author: String,
    /// Maintainer (email).
    pub maintainer: String,
    /// One-line synopsis.
    pub synopsis: String,
    /// Whether the current text field is being edited.
    pub editing: bool,
    /// Text input buffer for the current field.
    pub input_buffer: String,
}

impl InitWizard {
    /// Create a new init wizard with auto-detected defaults.
    pub fn new() -> Self {
        let dir_name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my-project".to_string());

        let git_name = std::process::Command::new("git")
            .args(["config", "user.name"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let git_email = std::process::Command::new("git")
            .args(["config", "user.email"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        Self {
            step: InitStep::Name,
            name: dir_name.clone(),
            template: TemplateKind::LibAndExe,
            license: "MIT".to_string(),
            author: git_name,
            maintainer: git_email,
            synopsis: String::new(),
            editing: true,
            input_buffer: dir_name,
        }
    }
}

impl Default for InitWizard {
    fn default() -> Self {
        Self::new()
    }
}

impl InitWizard {
    /// Get the current field value based on the step.
    fn current_value(&self) -> &str {
        match self.step {
            InitStep::Name => &self.name,
            InitStep::License => &self.license,
            InitStep::Author => &self.author,
            InitStep::Synopsis => &self.synopsis,
            InitStep::Template | InitStep::Confirm => "",
        }
    }

    /// Commit the input buffer to the current field.
    pub fn commit_input(&mut self) {
        match self.step {
            InitStep::Name => self.name = self.input_buffer.clone(),
            InitStep::License => self.license = self.input_buffer.clone(),
            InitStep::Author => self.author = self.input_buffer.clone(),
            InitStep::Synopsis => self.synopsis = self.input_buffer.clone(),
            InitStep::Template | InitStep::Confirm => {}
        }
    }

    /// Load the current field's value into the input buffer.
    pub fn load_input(&mut self) {
        self.input_buffer = self.current_value().to_string();
        self.editing = matches!(
            self.step,
            InitStep::Name | InitStep::License | InitStep::Author | InitStep::Synopsis
        );
    }

    /// Cycle the template selection (for the Template step).
    pub fn cycle_template(&mut self) {
        let all = TemplateKind::all();
        let idx = all.iter().position(|k| *k == self.template).unwrap_or(0);
        self.template = all[(idx + 1) % all.len()];
    }
}

/// Maximum number of undo states to keep.
const MAX_UNDO_ENTRIES: usize = 50;

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
    /// Whether a quit confirmation dialog is showing.
    pub confirm_quit: bool,
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
    /// Receiver for events from an async build subprocess.
    pub build_rx: Option<mpsc::UnboundedReceiver<BuildEvent>>,
    /// Init wizard state (Some when the wizard is active).
    pub init_wizard: Option<InitWizard>,
    /// Last known modification time of the .cabal file on disk.
    pub last_file_mtime: Option<std::time::SystemTime>,
    /// Undo stack — previous source strings for Ctrl+Z.
    pub undo_stack: Vec<String>,
    /// Scroll offset for the build output view (controlled by mouse scroll).
    pub build_scroll: usize,
    /// Detected GHC version (e.g. "9.8.2"), if available.
    pub ghc_version: Option<String>,
    /// Parsed GHC diagnostics from the last completed build.
    pub build_diagnostics: Vec<cabalist_cabal::GhcDiagnostic>,
    /// Index of the currently selected diagnostic for navigation.
    pub selected_diagnostic: usize,
    /// Current Hackage search results (populated as the user types).
    pub search_results: Vec<cabalist_hackage::SearchResult>,
    /// Index of the highlighted result in the search popup.
    pub search_selected: usize,
    /// Hackage package index, loaded once at startup from cache.
    pub hackage_index: Option<cabalist_hackage::HackageIndex>,
    /// Whether we are in inline-edit mode for a metadata field.
    pub editing_metadata: bool,
    /// The text being edited for the current metadata field.
    pub metadata_edit_buffer: String,
    /// Parsed cabal.project file, if one exists in the project root.
    pub cabal_project: Option<cabalist_project::CabalProject>,
    /// Whether the deps view is in tree mode (vs flat list).
    pub deps_tree_mode: bool,
    /// Whether the deps view is filtering inline (/ key) vs adding (a key).
    pub deps_filter_active: bool,
    /// The filter query for inline dep filtering.
    pub deps_filter_query: String,
    /// Whether we are in inline-edit mode for a project field.
    pub editing_project_field: bool,
    /// The text being edited for the current project field.
    pub project_edit_buffer: String,
    /// Path to the cabal.project file, if it exists.
    pub cabal_project_path: Option<PathBuf>,
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

        let last_file_mtime = std::fs::metadata(&cabal_path)
            .and_then(|m| m.modified())
            .ok();

        let ghc_version = cabalist_ghc::versions::detect_ghc_version();

        let hackage_index = load_hackage_index_from_cache();
        let cabal_project = load_cabal_project(project_root);
        let cabal_project_path = if cabal_project.is_some() {
            Some(project_root.join("cabal.project"))
        } else {
            None
        };

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
            confirm_quit: false,
            status_message: None,
            search_query: String::new(),
            search_active: false,
            selected_index: 0,
            selected_component: 0,
            build_output: Vec::new(),
            build_running: false,
            build_rx: None,
            init_wizard: None,
            last_file_mtime,
            undo_stack: Vec::new(),
            build_scroll: 0,
            ghc_version,

            build_diagnostics: Vec::new(),
            selected_diagnostic: 0,
            search_results: Vec::new(),
            search_selected: 0,
            hackage_index,
            editing_metadata: false,
            metadata_edit_buffer: String::new(),
            cabal_project,
            deps_tree_mode: false,
            deps_filter_active: false,
            deps_filter_query: String::new(),
            editing_project_field: false,
            project_edit_buffer: String::new(),
            cabal_project_path,
        };

        app.refresh_lints();
        Ok(app)
    }

    /// Create a new `App` in init wizard mode (no .cabal file exists yet).
    pub fn new_for_init(cabal_path: PathBuf, theme: Theme) -> anyhow::Result<Self> {
        // Parse an empty source so the app has a valid parse result.
        let source = String::new();
        let parse_result = cabalist_parser::parse(&source);

        let project_root = cabal_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let config = cabalist_opinions::config::find_and_load_config(project_root);

        let ghc_version = cabalist_ghc::versions::detect_ghc_version();

        let hackage_index = load_hackage_index_from_cache();
        let cabal_project = load_cabal_project(project_root);
        let cabal_project_path = if cabal_project.is_some() {
            Some(project_root.join("cabal.project"))
        } else {
            None
        };

        let app = Self {
            cabal_path,
            source,
            parse_result,
            lints: Vec::new(),
            config,
            theme,
            current_view: View::Init,
            dirty: false,
            should_quit: false,
            confirm_quit: false,
            status_message: None,
            search_query: String::new(),
            search_active: false,
            selected_index: 0,
            selected_component: 0,
            build_output: Vec::new(),
            build_running: false,
            build_rx: None,
            init_wizard: Some(InitWizard::new()),
            last_file_mtime: None,
            undo_stack: Vec::new(),
            build_scroll: 0,
            ghc_version,

            build_diagnostics: Vec::new(),
            selected_diagnostic: 0,
            search_results: Vec::new(),
            search_selected: 0,
            hackage_index,
            editing_metadata: false,
            metadata_edit_buffer: String::new(),
            cabal_project,
            deps_tree_mode: false,
            deps_filter_active: false,
            deps_filter_query: String::new(),
            editing_project_field: false,
            project_edit_buffer: String::new(),
            cabal_project_path,
        };

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
        self.last_file_mtime = std::fs::metadata(&self.cabal_path)
            .and_then(|m| m.modified())
            .ok();
        self.set_status("Reloaded from disk");
        Ok(())
    }

    /// Write the current CST back to disk.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let rendered = self.parse_result.cst.render();
        std::fs::write(&self.cabal_path, &rendered)?;
        self.dirty = false;
        self.last_file_mtime = std::fs::metadata(&self.cabal_path)
            .and_then(|m| m.modified())
            .ok();
        self.set_status("Saved");
        Ok(())
    }

    /// Re-run the opinionated lints on the current AST, including
    /// filesystem-aware lints when a project root is available.
    pub fn refresh_lints(&mut self) {
        let ast = self.ast();
        let lint_config = self.config.lints.to_lint_config();
        let project_root = self
            .cabal_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        self.lints = cabalist_opinions::lints::run_all_lints_with_cst(&ast, Some(&self.parse_result.cst), &lint_config, project_root);
    }

    /// Set a transient status message.
    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some((msg.to_string(), Instant::now()));
    }

    /// Drain pending build events from the async subprocess channel.
    ///
    /// Call this on every tick of the event loop so that build output is
    /// displayed in near-real-time.
    pub fn drain_build_events(&mut self) {
        // Take the receiver out so we can mutate self freely.
        let mut rx = match self.build_rx.take() {
            Some(rx) => rx,
            None => return,
        };

        loop {
            match rx.try_recv() {
                Ok(BuildEvent::Line(line)) => {
                    self.build_output.push(line);
                }
                Ok(BuildEvent::Complete { success, duration }) => {
                    let status = if success { "succeeded" } else { "FAILED" };
                    self.build_output.push(String::new());
                    self.build_output
                        .push(format!("Build {status} in {:.1}s", duration.as_secs_f64()));
                    self.build_running = false;

                    // Parse diagnostics from the full build output.
                    let full_output = self.build_output.join("\n");
                    self.build_diagnostics =
                        cabalist_cabal::diagnostics::parse_diagnostics(&full_output);
                    self.selected_diagnostic = 0;

                    self.set_status(&format!("Build {status}"));

                    // If the Hackage index was updated, reload it from cache.
                    if self.build_output.iter().any(|l| l.contains("Index updated")) {
                        self.hackage_index = load_hackage_index_from_cache();
                    }

                    // Channel is done; don't put it back.
                    return;
                }
                Ok(BuildEvent::Error(e)) => {
                    self.build_output.push(format!("Error: {e}"));
                    self.build_running = false;
                    self.set_status(&format!("Build error: {e}"));
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No more events right now; put the receiver back.
                    self.build_rx = Some(rx);
                    return;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Sender was dropped without a Complete/Error — treat as error.
                    if self.build_running {
                        self.build_output
                            .push("Build process terminated unexpectedly.".to_string());
                        self.build_running = false;
                        self.set_status("Build terminated unexpectedly");
                    }
                    return;
                }
            }
        }
    }

    /// Spawn a `cabal build` subprocess. Output streams into `build_rx`.
    pub fn spawn_build(&mut self) {
        if self.build_running {
            self.set_status("A build is already running");
            return;
        }
        self.build_output.clear();
        self.build_output.push("Building...".to_string());
        self.build_running = true;
        self.build_diagnostics.clear();
        self.selected_diagnostic = 0;
        self.set_status("Building...");

        let working_dir = self
            .cabal_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let (tx, rx) = mpsc::unbounded_channel::<BuildEvent>();
        self.build_rx = Some(rx);

        tokio::spawn(async move {
            // Create a line-level channel that the cabal runner streams into.
            let (line_tx, mut line_rx) = mpsc::unbounded_channel::<cabalist_cabal::OutputLine>();

            // Forward OutputLine values to our BuildEvent channel.
            let tx_fwd = tx.clone();
            tokio::spawn(async move {
                while let Some(line) = line_rx.recv().await {
                    let text = match line {
                        cabalist_cabal::OutputLine::Stdout(s) => s,
                        cabalist_cabal::OutputLine::Stderr(s) => s,
                    };
                    if tx_fwd.send(BuildEvent::Line(text)).is_err() {
                        break;
                    }
                }
            });

            let result = cabalist_cabal::cabal_build(
                &working_dir,
                &cabalist_cabal::BuildOptions::default(),
                Some(line_tx),
            )
            .await;

            match result {
                Ok(r) => {
                    let _ = tx.send(BuildEvent::Complete {
                        success: r.success,
                        duration: r.output.duration,
                    });
                }
                Err(e) => {
                    let _ = tx.send(BuildEvent::Error(e.to_string()));
                }
            }
        });
    }

    /// Spawn a `cabal test` subprocess. Output streams into `build_rx`.
    pub fn spawn_test(&mut self) {
        if self.build_running {
            self.set_status("A build is already running");
            return;
        }
        self.build_output.clear();
        self.build_output.push("Running tests...".to_string());
        self.build_running = true;
        self.build_diagnostics.clear();
        self.selected_diagnostic = 0;
        self.set_status("Running tests...");

        let working_dir = self
            .cabal_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let (tx, rx) = mpsc::unbounded_channel::<BuildEvent>();
        self.build_rx = Some(rx);

        tokio::spawn(async move {
            let (line_tx, mut line_rx) = mpsc::unbounded_channel::<cabalist_cabal::OutputLine>();

            let tx_fwd = tx.clone();
            tokio::spawn(async move {
                while let Some(line) = line_rx.recv().await {
                    let text = match line {
                        cabalist_cabal::OutputLine::Stdout(s) => s,
                        cabalist_cabal::OutputLine::Stderr(s) => s,
                    };
                    if tx_fwd.send(BuildEvent::Line(text)).is_err() {
                        break;
                    }
                }
            });

            let result = cabalist_cabal::cabal_test(
                &working_dir,
                &cabalist_cabal::BuildOptions::default(),
                Some(line_tx),
            )
            .await;

            match result {
                Ok(r) => {
                    let _ = tx.send(BuildEvent::Complete {
                        success: r.success,
                        duration: r.output.duration,
                    });
                }
                Err(e) => {
                    let _ = tx.send(BuildEvent::Error(e.to_string()));
                }
            }
        });
    }

    /// Spawn a `cabal clean` subprocess. Output streams into `build_rx`.
    pub fn spawn_clean(&mut self) {
        if self.build_running {
            self.set_status("A build is already running");
            return;
        }
        self.build_output.clear();
        self.build_output.push("Cleaning...".to_string());
        self.build_running = true;
        self.build_diagnostics.clear();
        self.selected_diagnostic = 0;
        self.set_status("Cleaning...");

        let working_dir = self
            .cabal_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let (tx, rx) = mpsc::unbounded_channel::<BuildEvent>();
        self.build_rx = Some(rx);

        tokio::spawn(async move {
            let result = cabalist_cabal::cabal_clean(&working_dir).await;

            match result {
                Ok(()) => {
                    let _ = tx.send(BuildEvent::Complete {
                        success: true,
                        duration: std::time::Duration::ZERO,
                    });
                }
                Err(e) => {
                    let _ = tx.send(BuildEvent::Error(e.to_string()));
                }
            }
        });
    }

    /// Total number of items in the current list (for bounds checking).
    pub fn current_list_len(&self) -> usize {
        match self.current_view {
            View::Metadata => 13, // number of metadata fields
            View::Dependencies => {
                let ast = self.ast();
                if self.deps_tree_mode {
                    crate::views::deps::tree_mode_item_count(&ast)
                } else {
                    count_deps_for_component(&ast, self.selected_component)
                }
            }
            View::Extensions => self.extensions_list_len(),
            View::Project => self
                .cabal_project
                .as_ref()
                .map(crate::views::project::item_count)
                .unwrap_or(0),
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
        // Push current state for undo.
        self.undo_stack.push(self.source.clone());
        if self.undo_stack.len() > MAX_UNDO_ENTRIES {
            self.undo_stack.remove(0);
        }

        let mut batch = EditBatch::new();
        batch.add_all(edits);
        self.source = batch.apply(&self.source);
        self.parse_result = cabalist_parser::parse(&self.source);
        self.refresh_lints();
        self.dirty = true;
    }

    /// Undo the last edit by restoring the previous source from the undo stack.
    pub fn undo(&mut self) -> Result<(), String> {
        let prev = self.undo_stack.pop().ok_or("Nothing to undo")?;
        self.source = prev;
        self.parse_result = cabalist_parser::parse(&self.source);
        self.refresh_lints();
        self.dirty = true;
        self.set_status("Undone");
        Ok(())
    }

    /// Set a top-level metadata field to a new value.
    ///
    /// If the field already exists in the CST, its value is replaced. If it
    /// does not exist, a new field is inserted at the root level.
    pub fn set_metadata_field(&mut self, field_name: &str, value: &str) -> Result<(), String> {
        let cst = &self.parse_result.cst;
        let root = cst.root;

        let edits = if let Some(field_id) = edit::find_field(cst, root, field_name) {
            vec![edit::set_field_value(cst, field_id, value)]
        } else {
            vec![edit::add_field_to_root(cst, field_name, value)]
        };

        self.apply_edits(edits);
        Ok(())
    }

    /// Check if the .cabal file has been modified externally, and reload if so.
    ///
    /// Does nothing if there are unsaved changes (dirty flag is set).
    pub fn check_file_changed(&mut self) {
        if self.dirty {
            return; // Don't clobber unsaved changes.
        }
        let mtime = std::fs::metadata(&self.cabal_path)
            .and_then(|m| m.modified())
            .ok();
        if mtime != self.last_file_mtime && self.last_file_mtime.is_some() && self.reload().is_ok()
        {
            self.set_status("File changed on disk -- reloaded");
        }
        self.last_file_mtime = mtime;
    }

    /// Start the init wizard (callable from the dashboard via 'i' key).
    pub fn start_init_wizard(&mut self) {
        self.init_wizard = Some(InitWizard::new());
        self.current_view = View::Init;
    }

    /// Finalize the init wizard: render the template, write files, reload.
    pub fn finalize_init(&mut self) -> Result<(), String> {
        let wizard = self
            .init_wizard
            .as_ref()
            .ok_or_else(|| "No init wizard active".to_string())?;

        let module_name = to_module_name(&wizard.name);

        let vars = cabalist_opinions::templates::TemplateVars {
            name: wizard.name.clone(),
            version: "0.1.0.0".to_string(),
            synopsis: if wizard.synopsis.is_empty() {
                "A short synopsis".to_string()
            } else {
                wizard.synopsis.clone()
            },
            description: "A longer description".to_string(),
            license: wizard.license.clone(),
            author: wizard.author.clone(),
            maintainer: wizard.maintainer.clone(),
            category: "Development".to_string(),
            repo_url: String::new(),
            language: self
                .ghc_version
                .as_deref()
                .map(cabalist_opinions::defaults::language_for_ghc_version)
                .unwrap_or(cabalist_opinions::DEFAULT_LANGUAGE)
                .to_string(),
            exposed_modules: module_name.clone(),
            base_version: self
                .ghc_version
                .as_deref()
                .and_then(|v| {
                    let map = cabalist_ghc::versions::ghc_base_map();
                    let mut best: Option<&cabalist_ghc::GhcBaseMapping> = None;
                    for entry in map {
                        if cabalist_ghc::versions::version_gte(v, entry.ghc) {
                            match best {
                                Some(prev) if cabalist_ghc::versions::version_gte(entry.ghc, prev.ghc) => best = Some(entry),
                                None => best = Some(entry),
                                _ => {}
                            }
                        }
                    }
                    best.map(|e| {
                        let parts: Vec<&str> = e.base.split('.').collect();
                        if parts.len() >= 2 { format!("{}.{}", parts[0], parts[1]) } else { e.base.to_string() }
                    })
                })
                .unwrap_or_else(|| "4.20".to_string()),
        };

        let template_kind = wizard.template;
        let cabal_content = cabalist_opinions::templates::render_template(template_kind, &vars);

        // Update the cabal path to use the project name.
        let project_dir = self
            .cabal_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let cabal_filename = format!("{}.cabal", wizard.name);
        self.cabal_path = project_dir.join(&cabal_filename);

        // Write the .cabal file.
        std::fs::write(&self.cabal_path, &cabal_content)
            .map_err(|e| format!("Failed to write .cabal file: {e}"))?;

        // Create directories and stub files based on template kind.
        create_project_dirs(&project_dir, template_kind, &wizard.name, &module_name)?;

        // Reload the app from the new file.
        self.source = cabal_content;
        self.parse_result = cabalist_parser::parse(&self.source);
        self.refresh_lints();
        self.dirty = false;
        self.last_file_mtime = std::fs::metadata(&self.cabal_path)
            .and_then(|m| m.modified())
            .ok();
        self.init_wizard = None;
        self.current_view = View::Dashboard;
        self.set_status(&format!("Created project '{}'", vars.name));

        Ok(())
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

    /// Toggle an extension in the selected component's default-extensions field.
    pub fn toggle_extension(&mut self, ext_name: &str) -> Result<(), String> {
        let (keyword, name) = self
            .selected_component_spec()
            .ok_or_else(|| "No component selected".to_string())?;
        let keyword = keyword.to_string();
        let name = name.map(|n| n.to_string());

        let cst = &self.parse_result.cst;
        let section_id = edit::find_section(cst, &keyword, name.as_deref())
            .ok_or_else(|| format!("Component '{keyword}' not found"))?;

        let ast = self.ast();
        let is_enabled = ast
            .all_components()
            .iter()
            .find(|c| {
                let f = c.fields();
                match (keyword.as_str(), name.as_deref()) {
                    ("library", _) => matches!(c, ast::Component::Library(_)),
                    (_, Some(n)) => f.name == Some(n),
                    _ => false,
                }
            })
            .map(|c| {
                c.fields()
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

    /// Update search results from the Hackage index based on the current query.
    pub fn update_search_results(&mut self) {
        if let Some(ref index) = self.hackage_index {
            if self.search_query.len() >= 2 {
                let results = index.search(&self.search_query);
                self.search_results = results.into_iter().take(10).collect();
            } else {
                self.search_results.clear();
            }
        } else {
            self.search_results.clear();
        }
        self.search_selected = 0;
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

    /// Spawn an async task to download/refresh the Hackage package index.
    /// The result is stored back into `hackage_index` when complete.
    pub fn spawn_hackage_update(&mut self) {
        if self.build_running {
            self.set_status("Another task is running — wait for it to finish");
            return;
        }
        self.set_status("Updating Hackage index...");

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<BuildEvent>();
        self.build_rx = Some(rx);
        self.build_running = true;

        tokio::spawn(async move {
            let cache_dir = match directories::ProjectDirs::from("", "", "cabalist") {
                Some(dirs) => dirs.cache_dir().to_path_buf(),
                None => {
                    let _ = tx.send(BuildEvent::Error(
                        "Could not determine cache directory".to_string(),
                    ));
                    return;
                }
            };

            let _ = tx.send(BuildEvent::Line(
                "Downloading Hackage index (~150MB)...".to_string(),
            ));

            match cabalist_hackage::client::update_index(&cache_dir).await {
                Ok(index) => {
                    let count = index.len();
                    let _ = tx.send(BuildEvent::Line(format!(
                        "Index updated: {count} packages"
                    )));
                    let _ = tx.send(BuildEvent::Complete {
                        success: true,
                        duration: std::time::Duration::ZERO,
                    });
                }
                Err(e) => {
                    let _ = tx.send(BuildEvent::Error(format!(
                        "Failed to update index: {e}"
                    )));
                }
            }
        });
    }


    /// Format the .cabal file (round-trip through parser, optional sort).
    pub fn format_file(&mut self) -> Result<(), String> {
        let project_root = self
            .cabal_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let config = cabalist_opinions::config::find_and_load_config(project_root);

        let mut current = self.source.clone();

        // Sort dependencies if configured.
        if config.formatting.sort_dependencies {
            current = cabalist_opinions::fmt::sort_list_field(&current, "build-depends");
        }
        if config.formatting.sort_modules {
            current = cabalist_opinions::fmt::sort_list_field(&current, "exposed-modules");
            current = cabalist_opinions::fmt::sort_list_field(&current, "other-modules");
        }

        // Round-trip through parser to normalize.
        let result = cabalist_parser::parse(&current);
        let formatted = result.cst.render();

        if formatted == self.source {
            self.set_status("Already formatted");
            return Ok(());
        }

        self.undo_stack.push(self.source.clone());
        self.source = formatted;
        self.parse_result = cabalist_parser::parse(&self.source);
        self.refresh_lints();
        self.dirty = true;
        self.set_status("Formatted");
        Ok(())
    }

    /// Count unlisted .hs files in the project's source directories.
    pub fn count_unlisted_modules(&self) -> usize {
        let project_root = self
            .cabal_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let ast = self.ast();

        let mut total = 0;
        for comp in ast.all_components() {
            let fields = comp.fields();
            let source_dirs: Vec<&str> = if fields.hs_source_dirs.is_empty() {
                vec!["."]
            } else {
                fields.hs_source_dirs.clone()
            };

            let mut listed: Vec<String> = Vec::new();
            if let cabalist_parser::ast::Component::Library(lib) = &comp {
                listed.extend(lib.exposed_modules.iter().map(|s| s.to_string()));
            }
            listed.extend(fields.other_modules.iter().map(|s| s.to_string()));

            for src_dir in &source_dirs {
                let dir = project_root.join(src_dir);
                if dir.is_dir() {
                    total += count_unlisted_in_dir(&dir, &dir, &listed);
                }
            }
        }
        total
    }

    /// Set a field in the cabal.project file (simple line-based editing).
    pub fn set_project_field(&mut self, field_name: &str, value: &str) -> Result<(), String> {
        let path = self
            .cabal_project_path
            .as_ref()
            .ok_or_else(|| "No cabal.project file".to_string())?;

        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read cabal.project: {e}"))?;

        let new_source = set_project_field_in_source(&source, field_name, value);

        std::fs::write(path, &new_source)
            .map_err(|e| format!("Failed to write cabal.project: {e}"))?;

        self.cabal_project = Some(cabalist_project::parse(&new_source));
        self.set_status(&format!("Updated {field_name} in cabal.project"));
        Ok(())
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

/// Convert a kebab-case project name to a PascalCase module name.
fn to_module_name(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Create project directories and stub files for a given template.
fn create_project_dirs(
    project_dir: &Path,
    template: TemplateKind,
    _project_name: &str,
    module_name: &str,
) -> Result<(), String> {
    let mkdir = |dir: &Path| {
        std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create {}: {e}", dir.display()))
    };

    let write_if_missing = |path: &Path, content: &str| -> Result<(), String> {
        if !path.exists() {
            std::fs::write(path, content)
                .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
        }
        Ok(())
    };

    let has_lib = matches!(
        template,
        TemplateKind::Library | TemplateKind::LibAndExe | TemplateKind::Full
    );
    let has_exe = matches!(
        template,
        TemplateKind::Application | TemplateKind::LibAndExe | TemplateKind::Full
    );
    let has_test = matches!(template, TemplateKind::Full);
    let has_bench = matches!(template, TemplateKind::Full);

    if has_lib {
        let src = project_dir.join("src");
        mkdir(&src)?;
        let lib_file = src.join(format!("{module_name}.hs"));
        write_if_missing(
            &lib_file,
            &format!(
                "module {module_name}\n  ( someFunc\n  ) where\n\nsomeFunc :: IO ()\nsomeFunc = putStrLn \"someFunc\"\n"
            ),
        )?;
    }

    if has_exe {
        let app_dir = project_dir.join("app");
        mkdir(&app_dir)?;
        let main_file = app_dir.join("Main.hs");
        let main_content = if has_lib {
            format!(
                "module Main (main) where\n\nimport {module_name} (someFunc)\n\nmain :: IO ()\nmain = someFunc\n"
            )
        } else {
            "module Main (main) where\n\nmain :: IO ()\nmain = putStrLn \"Hello, Haskell!\"\n"
                .to_string()
        };
        write_if_missing(&main_file, &main_content)?;
    }

    if has_test {
        let test_dir = project_dir.join("test");
        mkdir(&test_dir)?;
        let test_file = test_dir.join("Main.hs");
        write_if_missing(
            &test_file,
            "module Main (main) where\n\nmain :: IO ()\nmain = putStrLn \"Test suite not yet implemented\"\n",
        )?;
    }

    if has_bench {
        let bench_dir = project_dir.join("bench");
        mkdir(&bench_dir)?;
        let bench_file = bench_dir.join("Main.hs");
        write_if_missing(
            &bench_file,
            "module Main (main) where\n\nmain :: IO ()\nmain = putStrLn \"Benchmark not yet implemented\"\n",
        )?;
    }

    Ok(())
}

/// Set a field value in cabal.project source text.
///
/// If the field exists, replaces its value. If not, appends it.
fn set_project_field_in_source(source: &str, field_name: &str, value: &str) -> String {
    let field_lower = format!("{}:", field_name.to_lowercase());
    let lines: Vec<&str> = source.lines().collect();
    let mut result_lines: Vec<String> = Vec::new();
    let mut found = false;
    let mut skip_continuation = false;

    for line in &lines {
        if skip_continuation {
            // Continuation lines are indented (start with whitespace) and non-empty.
            let is_continuation = !line.is_empty()
                && line.starts_with([' ', '\t'])
                && !line.trim().is_empty();
            if is_continuation {
                continue; // Skip this continuation line.
            }
            skip_continuation = false;
        }

        let trimmed = line.trim_start();
        let lower = trimmed.to_lowercase();

        // Match the exact field name followed by a colon (not a prefix of another field).
        if !found && lower.starts_with(&field_lower) {
            // Verify it's an exact field match by checking what follows the colon.
            let after_prefix = &lower[field_lower.len()..];
            let is_exact = after_prefix.is_empty()
                || after_prefix.starts_with(' ')
                || after_prefix.starts_with('\t');
            if is_exact {
                result_lines.push(format!("{field_name}: {value}"));
                found = true;
                skip_continuation = true; // Skip any continuation lines of the old value.
                continue;
            }
        }

        result_lines.push(line.to_string());
    }

    if !found {
        result_lines.push(format!("{field_name}: {value}"));
    }

    let mut result = result_lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Try to load and parse a `cabal.project` file from the given project root.
///
/// Returns `None` if no `cabal.project` file exists.
fn load_cabal_project(project_root: &std::path::Path) -> Option<cabalist_project::CabalProject> {
    cabalist_project::find_project_file(project_root)
        .and_then(|path| cabalist_project::parse_file(&path).ok())
}

/// Try to load the Hackage index from the platform cache directory.
///
/// Returns `None` if the cache file does not exist or cannot be read.
fn load_hackage_index_from_cache() -> Option<cabalist_hackage::HackageIndex> {
    let dirs = directories::ProjectDirs::from("", "", "cabalist")?;
    let cache_path = dirs.cache_dir().join("index.json");
    cabalist_hackage::HackageIndex::load_from_cache(&cache_path).ok()
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

/// Recursively count .hs files not listed in the module lists.
fn count_unlisted_in_dir(
    base: &std::path::Path,
    dir: &std::path::Path,
    listed: &[String],
) -> usize {
    let mut count = 0;
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            count += count_unlisted_in_dir(base, &path, listed);
        } else if path.extension().is_some_and(|ext| ext == "hs") {
            if let Some(module_name) = path_to_module_name(base, &path) {
                if !listed.iter().any(|m| m == &module_name) {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Convert a .hs file path to a Haskell module name.
fn path_to_module_name(base: &std::path::Path, file: &std::path::Path) -> Option<String> {
    let relative = file.strip_prefix(base).ok()?;
    let stem = relative.with_extension("");
    let components: Vec<&str> = stem
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    if components.is_empty() {
        return None;
    }
    Some(components.join("."))
}
