//! Key event handling and action mapping.

use crate::app::App;
use crate::views::View;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions the event loop can dispatch after processing a key event.
pub enum Action {
    /// Do nothing.
    None,
    /// Quit the application.
    Quit,
    /// Switch to a different view.
    SwitchView(View),
    /// Save the .cabal file to disk.
    Save,
    /// Reload the .cabal file from disk.
    Reload,
    /// Undo the last edit.
    Undo,
    /// Move selection up.
    MoveUp,
    /// Move selection down.
    MoveDown,
    /// Move to the top of the list.
    MoveToTop,
    /// Move to the bottom of the list.
    MoveToBottom,
    /// Confirm / select current item.
    Select,
    /// Go back / close popup.
    Back,
    /// Switch to the next component tab.
    NextComponent,
    /// Switch to the previous component tab.
    PrevComponent,
    /// Toggle the search popup.
    ToggleSearch,
    /// Type a character into the search field.
    SearchInput(char),
    /// Delete a character from the search field.
    SearchBackspace,
    /// Add an item (dependency, extension).
    AddItem,
    /// Remove the selected item.
    RemoveItem,
    /// Toggle the selected item (extension on/off).
    ToggleItem,
    /// Show info popup for the selected item.
    ShowInfo,
    /// Trigger a build.
    Build,
    /// Trigger tests.
    Test,
    /// Trigger clean.
    Clean,
    /// Show the help overlay.
    ShowHelp,
    /// Start the init wizard.
    StartInit,
    /// Init wizard: type a character into the input buffer.
    InitInput(char),
    /// Init wizard: delete a character from the input buffer.
    InitBackspace,
    /// Init wizard: confirm current step / advance.
    InitConfirm,
    /// Init wizard: go back to previous step.
    InitBack,
    /// Init wizard: cycle option (for Template step).
    InitCycleOption,
    /// Navigate to the next build diagnostic.
    NextDiagnostic,
    /// Navigate to the previous build diagnostic.
    PrevDiagnostic,
}

/// Map a key event to an action based on current app state.
pub fn handle_key(app: &App, key: KeyEvent) -> Action {
    // If the init wizard is active, route to init key handler.
    if app.current_view == View::Init && app.init_wizard.is_some() {
        return handle_init_key(app, key);
    }

    // If search is active, route most keys to search input.
    if app.search_active {
        return handle_search_key(key);
    }

    // Global keybindings.
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Action::Quit,
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => return Action::Save,
        (KeyModifiers::CONTROL, KeyCode::Char('r')) => return Action::Reload,
        (KeyModifiers::CONTROL, KeyCode::Char('z')) => return Action::Undo,
        _ => {}
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::ShowHelp,
        KeyCode::Esc => Action::Back,
        // Navigation.
        KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
        KeyCode::Char('g') => Action::MoveToTop,
        KeyCode::Char('G') => Action::MoveToBottom,
        KeyCode::Enter => Action::Select,
        KeyCode::Tab => Action::NextComponent,
        KeyCode::BackTab => Action::PrevComponent,
        KeyCode::Char('/') => Action::ToggleSearch,
        // View-specific actions depend on the current view.
        _ => handle_view_key(app, key),
    }
}

/// Handle keys specific to the current view.
fn handle_view_key(app: &App, key: KeyEvent) -> Action {
    match app.current_view {
        View::Dashboard => handle_dashboard_key(key),
        View::Dependencies => handle_deps_key(key),
        View::Extensions => handle_extensions_key(key),
        View::Build => handle_build_key(key),
        View::Metadata => Action::None,
        View::Help => {
            // Any key closes the help overlay.
            Action::Back
        }
        View::Init => Action::None, // handled above
    }
}

fn handle_dashboard_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('d') => Action::SwitchView(View::Dependencies),
        KeyCode::Char('e') => Action::SwitchView(View::Extensions),
        KeyCode::Char('b') => Action::SwitchView(View::Build),
        KeyCode::Char('m') => Action::SwitchView(View::Metadata),
        KeyCode::Char('i') => Action::StartInit,
        _ => Action::None,
    }
}

fn handle_deps_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('a') => Action::AddItem,
        KeyCode::Char('r') => Action::RemoveItem,
        _ => Action::None,
    }
}

fn handle_extensions_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char(' ') => Action::ToggleItem,
        KeyCode::Char('i') => Action::ShowInfo,
        _ => Action::None,
    }
}

fn handle_build_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('b') => Action::Build,
        KeyCode::Char('t') => Action::Test,
        KeyCode::Char('c') => Action::Clean,
        KeyCode::Char('n') | KeyCode::Char(']') => Action::NextDiagnostic,
        KeyCode::Char('p') | KeyCode::Char('[') => Action::PrevDiagnostic,
        _ => Action::None,
    }
}

fn handle_search_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Enter => Action::Select,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(c) => Action::SearchInput(c),
        _ => Action::None,
    }
}

fn handle_init_key(app: &App, key: KeyEvent) -> Action {
    // Ctrl+C always quits.
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    let wizard = match app.init_wizard.as_ref() {
        Some(w) => w,
        None => return Action::None,
    };

    match key.code {
        KeyCode::Esc => Action::InitBack,
        KeyCode::Enter => Action::InitConfirm,
        KeyCode::Tab => {
            if wizard.step == crate::app::InitStep::Template {
                Action::InitCycleOption
            } else {
                Action::None
            }
        }
        KeyCode::Backspace => {
            if wizard.editing {
                Action::InitBackspace
            } else {
                Action::None
            }
        }
        KeyCode::Char(c) => {
            if wizard.editing {
                Action::InitInput(c)
            } else {
                Action::None
            }
        }
        _ => Action::None,
    }
}
