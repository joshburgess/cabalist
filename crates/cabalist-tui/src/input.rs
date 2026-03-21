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
    /// Start editing the currently selected metadata field.
    MetadataStartEdit,
    /// Type a character into the metadata edit buffer.
    MetadataInput(char),
    /// Delete a character from the metadata edit buffer.
    MetadataBackspace,
    /// Confirm the metadata edit (write the new value).
    MetadataConfirm,
    /// Cancel the metadata edit.
    MetadataCancel,
    /// Toggle between flat list and tree view for dependencies.
    ToggleDepsTreeMode,
    /// Trigger async Hackage index update.
    UpdateHackageIndex,
    /// Format the .cabal file (round-trip + optional sort).
    FormatFile,
    /// Toggle inline filter mode for deps view.
    ToggleDepsFilter,
    /// Type a character into the deps filter.
    DepsFilterInput(char),
    /// Delete a character from the deps filter.
    DepsFilterBackspace,
    /// Start editing a project field.
    ProjectStartEdit,
    /// Type a character into the project edit buffer.
    ProjectInput(char),
    /// Delete a character from the project edit buffer.
    ProjectBackspace,
    /// Confirm the project field edit.
    ProjectConfirm,
    /// Cancel the project field edit.
    ProjectCancel,
}

/// Map a key event to an action based on current app state.
pub fn handle_key(app: &App, key: KeyEvent) -> Action {
    // If the init wizard is active, route to init key handler.
    if app.current_view == View::Init && app.init_wizard.is_some() {
        return handle_init_key(app, key);
    }

    // If metadata inline edit is active, route to metadata edit handler.
    if app.editing_metadata {
        return handle_metadata_edit_key(key);
    }

    // If project field edit is active, route to project edit handler.
    if app.editing_project_field {
        return handle_project_edit_key(key);
    }

    // If deps filter is active, route keys to filter input.
    if app.deps_filter_active {
        return handle_deps_filter_key(key);
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
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => return Action::UpdateHackageIndex,
        (KeyModifiers::CONTROL, KeyCode::Char('f')) => return Action::FormatFile,
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
        // View-specific actions depend on the current view.
        // Note: '/' is handled per-view (deps uses filter, others use search).
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
        View::Metadata => handle_metadata_key(key),
        View::Project => handle_project_key(key),
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
        KeyCode::Char('p') => Action::SwitchView(View::Project),
        KeyCode::Char('i') => Action::StartInit,
        _ => Action::None,
    }
}

fn handle_deps_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('a') => Action::AddItem,
        KeyCode::Char('r') => Action::RemoveItem,
        KeyCode::Char('v') => Action::ToggleDepsTreeMode,
        KeyCode::Char('/') => Action::ToggleDepsFilter,
        _ => Action::None,
    }
}

fn handle_deps_filter_key(key: KeyEvent) -> Action {
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Esc => Action::ToggleDepsFilter,
        KeyCode::Backspace => Action::DepsFilterBackspace,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
        KeyCode::Char(c) => Action::DepsFilterInput(c),
        _ => Action::None,
    }
}

fn handle_extensions_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char(' ') => Action::ToggleItem,
        KeyCode::Char('i') => Action::ShowInfo,
        KeyCode::Char('/') => Action::ToggleSearch,
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

/// Handle keys in the metadata view when NOT in inline-edit mode.
fn handle_metadata_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::MetadataStartEdit,
        _ => Action::None,
    }
}

/// Handle keys when inline-editing a metadata field value.
fn handle_metadata_edit_key(key: KeyEvent) -> Action {
    // Ctrl+C still quits.
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    match key.code {
        KeyCode::Esc => Action::MetadataCancel,
        KeyCode::Enter => Action::MetadataConfirm,
        KeyCode::Backspace => Action::MetadataBackspace,
        KeyCode::Char(c) => Action::MetadataInput(c),
        _ => Action::None,
    }
}

fn handle_project_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Enter => Action::ProjectStartEdit,
        _ => Action::None,
    }
}

fn handle_project_edit_key(key: KeyEvent) -> Action {
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Esc => Action::ProjectCancel,
        KeyCode::Enter => Action::ProjectConfirm,
        KeyCode::Backspace => Action::ProjectBackspace,
        KeyCode::Char(c) => Action::ProjectInput(c),
        _ => Action::None,
    }
}

fn handle_search_key(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Enter => Action::Select,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Up => Action::MoveUp,
        KeyCode::Down => Action::MoveDown,
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
