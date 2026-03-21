//! View system — each view is a full-screen rendering mode.

pub mod build;
pub mod dashboard;
pub mod deps;
pub mod extensions;
pub mod help;
pub mod init;
pub mod metadata;
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
