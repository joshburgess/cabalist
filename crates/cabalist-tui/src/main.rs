//! cabalist — an interactive TUI for managing Haskell .cabal files.

mod app;
mod event;
mod input;
mod theme;
mod views;
mod widgets;

use app::App;
use clap::Parser;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyEventKind, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use event::{poll_event, AppEvent};
use input::Action;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Terminal;
use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;
use views::View;

#[derive(Parser, Debug)]
#[command(
    name = "cabalist",
    about = "Interactive TUI for managing Haskell .cabal files"
)]
struct Cli {
    /// Path to the .cabal file (auto-detected if not specified).
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Color theme: dark or light.
    #[arg(long, default_value = "dark")]
    theme: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let theme = match cli.theme.as_str() {
        "light" => theme::Theme::light(),
        _ => theme::Theme::dark(),
    };

    // Find or determine the .cabal file path.
    let (cabal_path, init_mode) = match cli.file {
        Some(path) => {
            if !path.exists() {
                anyhow::bail!("File not found: {}", path.display());
            }
            (path, false)
        }
        None => match find_cabal_file() {
            Ok(path) => (path, false),
            Err(_) => {
                // No .cabal file found — start in init wizard mode.
                let cwd = std::env::current_dir()?;
                let dir_name = cwd
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "new-project".to_string());
                (cwd.join(format!("{dir_name}.cabal")), true)
            }
        },
    };

    let mut app = if init_mode {
        App::new_for_init(cabal_path, theme)?
    } else {
        App::new(cabal_path, theme)?
    };

    // Set up a panic hook that restores the terminal before printing the
    // panic message.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(DisableMouseCapture);
        let _ = stdout().execute(LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Enter alternate screen, raw mode, and enable mouse capture.
    stdout()
        .execute(EnterAlternateScreen)?
        .execute(EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Main event loop.
    let tick_rate = Duration::from_millis(100);

    loop {
        // Render.
        terminal.draw(|frame| {
            render(frame, &app);
        })?;

        // Poll for events.
        match poll_event(tick_rate)? {
            AppEvent::Terminal(CrosstermEvent::Key(key)) => {
                // crossterm 0.28 fires Press and Release events; only handle Press.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let action = input::handle_key(&app, key);
                handle_action(&mut app, action);
            }
            AppEvent::Terminal(CrosstermEvent::Mouse(mouse)) => {
                handle_mouse(&mut app, mouse);
            }
            AppEvent::Terminal(CrosstermEvent::Resize(_, _)) => {
                // Terminal will re-render on next iteration.
            }
            AppEvent::Tick => {
                // Clear expired status messages.
                if let Some((_, ref instant)) = app.status_message {
                    if instant.elapsed().as_secs() >= 5 {
                        app.status_message = None;
                    }
                }
                // Check for external file changes.
                app.check_file_changed();
            }
            _ => {}
        }

        // Drain any pending build subprocess output.
        app.drain_build_events();

        if app.should_quit {
            break;
        }
    }

    // Restore terminal.
    disable_raw_mode()?;
    stdout()
        .execute(DisableMouseCapture)?
        .execute(LeaveAlternateScreen)?;

    Ok(())
}

/// Render the full application frame.
fn render(frame: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(0),    // main content
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    widgets::header::render(frame, app, chunks[0]);

    match app.current_view {
        View::Dashboard => views::dashboard::render(frame, app, chunks[1]),
        View::Dependencies => views::deps::render(frame, app, chunks[1]),
        View::Extensions => views::extensions::render(frame, app, chunks[1]),
        View::Build => views::build::render(frame, app, chunks[1]),
        View::Metadata => views::metadata::render(frame, app, chunks[1]),
        View::Help => {
            // Render the underlying dashboard first, then the help overlay.
            views::dashboard::render(frame, app, chunks[1]);
            views::help::render(frame, app, chunks[1]);
        }
        View::Init => views::init::render(frame, app, chunks[1]),
    }

    widgets::status_bar::render(frame, app, chunks[2]);

    // Render search popup overlay if active.
    if app.search_active {
        widgets::search::render(frame, app, chunks[1]);
    }
}

/// Apply an action to the app state.
fn handle_action(app: &mut App, action: Action) {
    match action {
        Action::None => {}
        Action::Quit => {
            app.should_quit = true;
        }
        Action::SwitchView(view) => {
            app.current_view = view;
            app.selected_index = 0;
            app.search_query.clear();
            app.search_active = false;
            app.search_results.clear();
            app.search_selected = 0;
        }
        Action::Save => {
            if let Err(e) = app.save() {
                app.set_status(&format!("Save failed: {e}"));
            }
        }
        Action::Reload => {
            if let Err(e) = app.reload() {
                app.set_status(&format!("Reload failed: {e}"));
            }
        }
        Action::Undo => {
            if let Err(e) = app.undo() {
                app.set_status(&e);
            }
        }
        Action::MoveUp => {
            if app.search_active {
                if app.search_selected > 0 {
                    app.search_selected -= 1;
                } else if !app.search_results.is_empty() {
                    app.search_selected = app.search_results.len() - 1;
                }
            } else if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }
        Action::MoveDown => {
            if app.search_active {
                if !app.search_results.is_empty() {
                    app.search_selected = (app.search_selected + 1) % app.search_results.len();
                }
            } else {
                let max = app.current_list_len().saturating_sub(1);
                if app.selected_index < max {
                    app.selected_index += 1;
                }
            }
        }
        Action::MoveToTop => {
            app.selected_index = 0;
        }
        Action::MoveToBottom => {
            app.selected_index = app.current_list_len().saturating_sub(1);
        }
        Action::Select => {
            if app.search_active && app.current_view == View::Dependencies {
                // User submitted search in deps view — add as dependency.
                let dep_str = if !app.search_results.is_empty() {
                    let selected = &app.search_results[app.search_selected];
                    if let Some(ref idx) = app.hackage_index {
                        if let Some(latest) = idx.latest_version(&selected.package.name) {
                            let bounds = cabalist_hackage::compute_pvp_bounds(latest);
                            format!("{} {bounds}", selected.package.name)
                        } else {
                            selected.package.name.clone()
                        }
                    } else {
                        selected.package.name.clone()
                    }
                } else {
                    app.search_query.trim().to_string()
                };
                app.search_active = false;
                app.search_query.clear();
                app.search_results.clear();
                app.search_selected = 0;
                if dep_str.is_empty() {
                    app.set_status("No package name entered");
                } else {
                    match app.add_dependency(&dep_str) {
                        Ok(()) => app.set_status(&format!("Added '{dep_str}'")),
                        Err(e) => app.set_status(&format!("Add failed: {e}")),
                    }
                }
            } else {
                app.search_active = false;
            }
        }
        Action::Back => {
            if app.search_active {
                app.search_active = false;
                app.search_query.clear();
                app.search_results.clear();
                app.search_selected = 0;
            } else if app.current_view == View::Help {
                app.current_view = View::Dashboard;
            } else if app.current_view != View::Dashboard {
                app.current_view = View::Dashboard;
                app.selected_index = 0;
            }
        }
        Action::NextComponent => {
            app.selected_component = app.selected_component.wrapping_add(1);
            app.selected_index = 0;
        }
        Action::PrevComponent => {
            app.selected_component = app.selected_component.wrapping_sub(1);
            app.selected_index = 0;
        }
        Action::ToggleSearch => {
            app.search_active = !app.search_active;
            if !app.search_active {
                app.search_query.clear();
                app.search_results.clear();
                app.search_selected = 0;
            }
        }
        Action::SearchInput(c) => {
            app.search_query.push(c);
            app.update_search_results();
        }
        Action::SearchBackspace => {
            app.search_query.pop();
            app.update_search_results();
        }
        Action::AddItem => {
            // Open search for adding a dependency.
            app.search_active = true;
            app.search_query.clear();
            app.search_results.clear();
            app.search_selected = 0;
            app.set_status("Type to search...");
        }
        Action::RemoveItem => {
            if app.current_view == View::Dependencies {
                if let Some(dep_name) = app.dep_name_at_index(app.selected_index) {
                    match app.remove_dependency(&dep_name) {
                        Ok(()) => {
                            app.set_status(&format!("Removed '{dep_name}'"));
                            // Adjust selection if we removed the last item.
                            let max = app.current_list_len().saturating_sub(1);
                            if app.selected_index > max {
                                app.selected_index = max;
                            }
                        }
                        Err(e) => app.set_status(&format!("Remove failed: {e}")),
                    }
                } else {
                    app.set_status("No dependency selected");
                }
            } else {
                app.set_status("Remove not available in this view");
            }
        }
        Action::ToggleItem => {
            if app.current_view == View::Extensions {
                if let Some((ext_name, _enabled)) = app.extension_at_index(app.selected_index) {
                    match app.toggle_extension(&ext_name) {
                        Ok(()) => app.set_status(&format!("Toggled '{ext_name}'")),
                        Err(e) => app.set_status(&format!("Toggle failed: {e}")),
                    }
                } else {
                    app.set_status("No extension selected");
                }
            } else {
                app.set_status("Toggle not available in this view");
            }
        }
        Action::ShowInfo => {
            if app.current_view == View::Extensions {
                if let Some((ext_name, enabled)) = app.extension_at_index(app.selected_index) {
                    let all_ext = cabalist_ghc::extensions::load_extensions();
                    if let Some(ext) = all_ext.iter().find(|e| e.name == ext_name) {
                        let status = if enabled { "enabled" } else { "available" };
                        app.set_status(&format!("{} ({}): {}", ext.name, status, ext.description));
                    } else {
                        app.set_status(&format!("{ext_name}: no info available"));
                    }
                } else {
                    app.set_status("No extension selected");
                }
            } else {
                app.set_status("Info not available in this view");
            }
        }
        Action::Build => {
            app.current_view = View::Build;
            app.spawn_build();
        }
        Action::Test => {
            app.current_view = View::Build;
            app.spawn_test();
        }
        Action::Clean => {
            app.current_view = View::Build;
            app.spawn_clean();
        }
        Action::ShowHelp => {
            if app.current_view == View::Help {
                app.current_view = View::Dashboard;
            } else {
                app.current_view = View::Help;
            }
        }
        Action::StartInit => {
            app.start_init_wizard();
        }
        Action::InitInput(c) => {
            if let Some(ref mut wizard) = app.init_wizard {
                wizard.input_buffer.push(c);
            }
        }
        Action::InitBackspace => {
            if let Some(ref mut wizard) = app.init_wizard {
                wizard.input_buffer.pop();
            }
        }
        Action::InitConfirm => {
            // Commit input and advance to next step, or finalize on Confirm.
            let at_confirm = app
                .init_wizard
                .as_ref()
                .map(|w| w.step == app::InitStep::Confirm)
                .unwrap_or(false);

            if at_confirm {
                match app.finalize_init() {
                    Ok(()) => {} // status set inside finalize_init
                    Err(e) => app.set_status(&format!("Init failed: {e}")),
                }
            } else if let Some(ref mut wizard) = app.init_wizard {
                wizard.commit_input();
                if let Some(next) = wizard.step.next() {
                    wizard.step = next;
                    wizard.load_input();
                }
            }
        }
        Action::InitBack => {
            let should_quit_init = app
                .init_wizard
                .as_ref()
                .map(|w| w.step == app::InitStep::Name)
                .unwrap_or(false);

            if should_quit_init {
                // On first step, Esc exits the wizard.
                app.init_wizard = None;
                // If we started in init mode (empty source), quit entirely.
                if app.source.is_empty() {
                    app.should_quit = true;
                } else {
                    app.current_view = View::Dashboard;
                }
            } else if let Some(ref mut wizard) = app.init_wizard {
                wizard.commit_input();
                if let Some(prev) = wizard.step.prev() {
                    wizard.step = prev;
                    wizard.load_input();
                }
            }
        }
        Action::InitCycleOption => {
            if let Some(ref mut wizard) = app.init_wizard {
                wizard.cycle_template();
            }
        }
        Action::NextDiagnostic => {
            if !app.build_diagnostics.is_empty() {
                app.selected_diagnostic =
                    (app.selected_diagnostic + 1) % app.build_diagnostics.len();
                scroll_to_diagnostic(app);
            } else {
                app.set_status("No diagnostics to navigate");
            }
        }
        Action::PrevDiagnostic => {
            if !app.build_diagnostics.is_empty() {
                if app.selected_diagnostic == 0 {
                    app.selected_diagnostic = app.build_diagnostics.len() - 1;
                } else {
                    app.selected_diagnostic -= 1;
                }
                scroll_to_diagnostic(app);
            } else {
                app.set_status("No diagnostics to navigate");
            }
        }
        Action::MetadataStartEdit => {
            if let Some(&field_name) = views::metadata::METADATA_FIELDS.get(app.selected_index) {
                // Load the current value into the edit buffer.
                let ast = app.ast();
                let current_value = match field_name {
                    "name" => ast.name.map(|s| s.to_string()),
                    "version" => ast.version.as_ref().map(|v| v.to_string()),
                    "cabal-version" => ast.cabal_version.as_ref().map(|cv| cv.raw.to_string()),
                    "license" => ast.license.map(|s| s.to_string()),
                    "author" => ast.author.map(|s| s.to_string()),
                    "maintainer" => ast.maintainer.map(|s| s.to_string()),
                    "homepage" => ast.homepage.map(|s| s.to_string()),
                    "bug-reports" => ast.bug_reports.map(|s| s.to_string()),
                    "synopsis" => ast.synopsis.map(|s| s.to_string()),
                    "description" => ast.description.map(|s| s.to_string()),
                    "category" => ast.category.map(|s| s.to_string()),
                    "build-type" => ast.build_type.map(|s| s.to_string()),
                    "tested-with" => ast.tested_with.map(|s| s.to_string()),
                    _ => None,
                };
                app.metadata_edit_buffer = current_value.unwrap_or_default();
                app.editing_metadata = true;
            }
        }
        Action::MetadataInput(c) => {
            app.metadata_edit_buffer.push(c);
        }
        Action::MetadataBackspace => {
            app.metadata_edit_buffer.pop();
        }
        Action::MetadataConfirm => {
            let field_name = views::metadata::METADATA_FIELDS
                .get(app.selected_index)
                .copied()
                .unwrap_or("");
            let value = app.metadata_edit_buffer.clone();
            app.editing_metadata = false;
            if !field_name.is_empty() {
                match app.set_metadata_field(field_name, &value) {
                    Ok(()) => app.set_status(&format!("Updated {field_name}")),
                    Err(e) => app.set_status(&format!("Failed: {e}")),
                }
            }
        }
        Action::MetadataCancel => {
            app.editing_metadata = false;
            app.metadata_edit_buffer.clear();
            app.set_status("Edit cancelled");
        }
    }
}

/// Scroll the build output so the currently selected diagnostic's line is visible.
fn scroll_to_diagnostic(app: &mut App) {
    let diag = &app.build_diagnostics[app.selected_diagnostic];
    let pattern = format!("{}:{}:{}", diag.file, diag.line, diag.column);

    // Find the line index in build_output that contains this diagnostic header.
    if let Some(line_idx) = app
        .build_output
        .iter()
        .position(|line| line.contains(&pattern))
    {
        app.build_scroll = line_idx;
    }

    let severity = match diag.severity {
        cabalist_cabal::GhcSeverity::Error => "error",
        cabalist_cabal::GhcSeverity::Warning => "warning",
    };
    let total = app.build_diagnostics.len();
    let current = app.selected_diagnostic + 1;
    app.set_status(&format!(
        "[{current}/{total}] {severity}: {}:{}:{} {}",
        diag.file, diag.line, diag.column, diag.message
    ));
}

/// Handle mouse events: click to select in lists, scroll wheel to navigate.
fn handle_mouse(app: &mut App, event: MouseEvent) {
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Click on list items to select them.
            // The main content area starts at row 1 (after header).
            // List items typically start around row 3-4 (inside a bordered block).
            let row = event.row as usize;

            match app.current_view {
                View::Dependencies | View::Extensions | View::Metadata => {
                    // The bordered block starts at row 1 (header takes row 0),
                    // the block title/border takes 1 row, so items start ~row 3.
                    let list_start_row = 3;
                    if row >= list_start_row {
                        let list_idx = row - list_start_row;
                        let max = app.current_list_len().saturating_sub(1);
                        app.selected_index = list_idx.min(max);
                    }
                }
                _ => {}
            }
        }
        MouseEventKind::ScrollUp => {
            if app.current_view == View::Build {
                // Scroll build output up.
                if app.build_scroll > 0 {
                    app.build_scroll -= 1;
                }
            } else if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }
        MouseEventKind::ScrollDown => {
            if app.current_view == View::Build {
                // Scroll build output down.
                app.build_scroll = app.build_scroll.saturating_add(1);
                // Clamp to valid range.
                let max_scroll = app.build_output.len();
                if app.build_scroll > max_scroll {
                    app.build_scroll = max_scroll;
                }
            } else {
                let max = app.current_list_len().saturating_sub(1);
                if app.selected_index < max {
                    app.selected_index += 1;
                }
            }
        }
        _ => {}
    }
}

/// Search for a `.cabal` file in the current directory.
fn find_cabal_file() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut cabal_files: Vec<PathBuf> = Vec::new();

    for entry in std::fs::read_dir(&cwd)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "cabal" {
                    cabal_files.push(path);
                }
            }
        }
    }

    match cabal_files.len() {
        0 => anyhow::bail!(
            "No .cabal file found in {}. Use --file to specify one.",
            cwd.display()
        ),
        1 => Ok(cabal_files.remove(0)),
        n => anyhow::bail!(
            "Found {n} .cabal files in {}. Use --file to specify which one.",
            cwd.display()
        ),
    }
}
