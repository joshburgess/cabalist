# TUI User Guide

The `cabalist` TUI is an interactive terminal application for managing Haskell `.cabal` files. It provides 8 views covering every aspect of `.cabal` file management.

## Launching

```sh
cabalist                    # Auto-detect .cabal file in current directory
cabalist --file my-lib.cabal  # Specify a .cabal file
cabalist --theme light        # Use the light color theme
```

If no `.cabal` file exists, the TUI starts in **Init Wizard** mode.

## Views

### Dashboard (Home)

The dashboard is the landing page. It shows:

- **Metadata**: Package name, version, license, synopsis, cabal-version with visual indicators (`+` set, `!` missing)
- **Components**: Library, executables, test-suites, benchmarks with module and dependency counts
- **Health**: Lint summary (errors, warnings, suggestions), outdated dependency count, unlisted module count, and the first 5 lint messages

**Navigation**: Press `d` (dependencies), `e` (extensions), `b` (build), `m` (metadata), `p` (project), or `i` (init wizard).

### Dependencies View

Shows `build-depends` for the currently selected component.

**List mode** (default): Each dependency shows its package name, version constraint, PVP bound status (`+ PVP ok` or `! no upper bound`), and an outdated indicator (`-> X.Y.Z` in red if the constraint excludes the latest Hackage version).

**Tree mode** (press `v`): ASCII tree showing all components and their dependencies with `├──` / `└──` connectors.

**Inline filter** (press `/`): Type to narrow the dependency list. The filter appears in the block title. Press `Esc` to clear.

| Key | Action |
|-----|--------|
| `a` | Add dependency (opens Hackage search popup) |
| `r` | Remove selected dependency |
| `v` | Toggle list/tree view |
| `/` | Inline filter |
| `Tab` | Switch component |

**Adding a dependency**: Press `a`, type to search Hackage. Results appear ranked by relevance. Press `Enter` to add the selected package with PVP bounds. Press `Esc` to cancel.

### Extensions View

Browse and toggle GHC language extensions. Shows enabled extensions first (marked `[x]`), then all available extensions (`[ ]`).

Each extension shows its name, recommended badge, and a truncated description. Press `i` for full details including GHC version, category, and safety information.

| Key | Action |
|-----|--------|
| `Space` | Toggle extension on/off |
| `i` | Show detailed info |
| `/` | Filter by name |

### Metadata View

Edit all 13 top-level `.cabal` fields:

`name`, `version`, `cabal-version`, `license`, `author`, `maintainer`, `homepage`, `bug-reports`, `synopsis`, `description`, `category`, `build-type`, `tested-with`

Fields show their current value with status indicators: `+` (set), `-` (unset), `>` (editing).

| Key | Action |
|-----|--------|
| `Enter` | Start editing the selected field |
| Type | Edit the value |
| `Enter` | Confirm the edit |
| `Esc` | Cancel the edit |

### Build View

Run `cabal build`, `cabal test`, and `cabal clean` with streaming output.

Build output scrolls as it arrives. When the build finishes, GHC diagnostics are parsed and navigable.

| Key | Action |
|-----|--------|
| `b` | Run `cabal build` |
| `t` | Run `cabal test` |
| `c` | Run `cabal clean` |
| `n` / `]` | Jump to next diagnostic |
| `p` / `[` | Jump to previous diagnostic |
| Mouse scroll | Scroll output |

### Project View

View and edit `cabal.project` file configuration. If no `cabal.project` exists, shows a placeholder message.

**Overview section**: Package count, compiler (`with-compiler`), index state (`index-state`), source repository package count.

**Details section**: Constraints, `allow-newer`, `allow-older`, source-repository-packages, package stanzas, and other fields.

The `with-compiler` and `index-state` fields are editable inline (press `Enter` to edit).

### Help Overlay

Press `?` from any view to see context-sensitive keybindings. Press any key to dismiss.

### Init Wizard

Guided project creation in 6 steps:

1. **Name** — defaults to the current directory name
2. **Template** — cycle with `Tab`: Library, Application, Library + Application, Full
3. **License** — type the license identifier (default: MIT)
4. **Author** — auto-detected from `git config user.name`
5. **Synopsis** — one-line description
6. **Confirm** — review and create

The wizard creates the `.cabal` file and project directory structure (`src/`, `app/`, `test/`, `bench/` as appropriate).

## Global Keybindings

These work in every view:

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `Ctrl+S` | Save to disk |
| `Ctrl+R` | Reload from disk |
| `Ctrl+Z` | Undo last edit |
| `Ctrl+F` | Format the .cabal file |
| `Ctrl+U` | Update Hackage index |
| `?` | Show help |
| `Esc` | Go back / close |
| `j`/`k` or arrows | Navigate |
| `g` / `G` | Jump to top / bottom |
| `Tab` / `Shift+Tab` | Switch component |

## File Watching

The TUI detects when the `.cabal` file changes on disk (e.g., from a git operation or another editor). If there are no unsaved changes, it automatically reloads. If there are unsaved changes, it preserves them.

## Undo

Every edit (add/remove dependency, toggle extension, edit metadata) pushes the previous state onto the undo stack. `Ctrl+Z` restores the previous state. The stack holds up to 50 entries.

## Hackage Index

The TUI uses a cached Hackage package index for dependency search, version comparison, and outdated detection. The cache is stored at `~/.cache/cabalist/index.json`.

Press `Ctrl+U` to download/refresh the index (~150MB download). Progress is shown in the build view. The index is automatically reloaded after a successful download.

## Themes

The TUI supports dark and light themes:

```sh
cabalist --theme dark    # Default
cabalist --theme light   # For light terminal backgrounds
```

## Mouse Support

- Click on list items to select them
- Scroll wheel navigates lists and build output
- Works in Dependencies, Extensions, Metadata, Build, and Project views
