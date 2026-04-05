//! View system — each view is a full-screen rendering mode.

/// Build output viewer with streaming log display.
pub mod build;
/// Home screen with project overview and health summary.
pub mod dashboard;
/// Dependency manager for build-depends.
pub mod deps;
/// GHC extension browser and toggler.
pub mod extensions;
/// Help overlay with keybinding reference.
pub mod help;
/// Init wizard for creating a new project.
pub mod init;
/// Metadata field editor (name, version, license, etc.).
pub mod metadata;
/// `cabal.project` file viewer and editor.
pub mod project;

/// The active view in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Home screen with project overview and health summary.
    Dashboard,
    /// Dependency manager for build-depends.
    Dependencies,
    /// GHC extension browser/toggler.
    Extensions,
    /// Build output viewer.
    Build,
    /// Metadata field editor.
    Metadata,
    /// cabal.project file viewer/editor.
    Project,
    /// Help overlay (renders on top of the current view).
    Help,
    /// Init wizard for creating a new project.
    Init,
}
