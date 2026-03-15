//! cabalist — an interactive TUI for managing Haskell .cabal files.

mod app;
mod event;
mod input;
mod theme;
mod views;
mod widgets;

use app::App;
use clap::Parser;
use crossterm::event::{Event as CrosstermEvent, KeyEventKind};
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

    // Find the .cabal file.
    let cabal_path = match cli.file {
        Some(path) => {
            if !path.exists() {
                anyhow::bail!("File not found: {}", path.display());
            }
            path
        }
        None => find_cabal_file()?,
    };

    let theme = match cli.theme.as_str() {
        "light" => theme::Theme::light(),
        _ => theme::Theme::dark(),
    };

    let mut app = App::new(cabal_path, theme)?;

    // Set up a panic hook that restores the terminal before printing the
    // panic message.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Enter alternate screen and raw mode.
    stdout().execute(EnterAlternateScreen)?;
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
    stdout().execute(LeaveAlternateScreen)?;

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
        Action::MoveUp => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            }
        }
        Action::MoveDown => {
            let max = app.current_list_len().saturating_sub(1);
            if app.selected_index < max {
                app.selected_index += 1;
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
                let dep_str = app.search_query.trim().to_string();
                app.search_active = false;
                app.search_query.clear();
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
            }
        }
        Action::SearchInput(c) => {
            app.search_query.push(c);
        }
        Action::SearchBackspace => {
            app.search_query.pop();
        }
        Action::AddItem => {
            // Open search for adding a dependency.
            app.search_active = true;
            app.search_query.clear();
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
