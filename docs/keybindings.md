# Keybindings

## Global (available in all views)

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Ctrl+C` | Quit |
| `Ctrl+S` | Save `.cabal` file to disk |
| `Ctrl+R` | Reload `.cabal` file from disk |
| `Ctrl+Z` | Undo last edit |
| `Ctrl+U` | Update Hackage index (download from network) |
| `?` | Show contextual help |
| `Esc` | Go back / close popup |
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `g` | Jump to top of list |
| `G` | Jump to bottom of list |
| `Enter` | Confirm / select current item |
| `Tab` | Switch to next component |
| `Shift+Tab` | Switch to previous component |

## Dashboard

| Key | Action |
|-----|--------|
| `d` | Switch to Dependencies view |
| `e` | Switch to Extensions view |
| `b` | Switch to Build view |
| `m` | Switch to Metadata view |
| `p` | Switch to Project view (cabal.project) |
| `i` | Start init wizard |

## Dependencies

| Key | Action |
|-----|--------|
| `a` | Add a dependency (opens Hackage search popup) |
| `r` | Remove the selected dependency |
| `v` | Toggle between list and tree view |
| `/` | Filter dependencies by name (inline) |
| `Tab` | Switch component |

## Extensions

| Key | Action |
|-----|--------|
| `Space` | Toggle the selected extension on/off |
| `i` | Show info about the selected extension |
| `/` | Filter extensions by name |

## Metadata

| Key | Action |
|-----|--------|
| `Enter` | Edit the selected field inline |
| `Esc` | Cancel editing |

## Project (cabal.project)

| Key | Action |
|-----|--------|
| `Enter` | Edit the selected field (compiler, index-state) |
| `Esc` | Cancel editing |

## Build Output

| Key | Action |
|-----|--------|
| `b` | Start a build (`cabal build`) |
| `t` | Run tests (`cabal test`) |
| `c` | Clean (`cabal clean`) |
| `n` / `]` | Jump to next error/warning |
| `p` / `[` | Jump to previous error/warning |
| Mouse scroll | Scroll build output |

## Dependency Filter

When filtering is active in the Dependencies view:

| Key | Action |
|-----|--------|
| Type | Narrow the dependency list |
| `Backspace` | Delete filter character |
| `Esc` | Clear filter and exit filter mode |
| `Up` / `Down` | Navigate filtered results |

## Search Popup (Hackage)

When the Hackage search popup is open (via `a` in Dependencies):

| Key | Action |
|-----|--------|
| Type | Search Hackage packages |
| `Backspace` | Delete character |
| `Enter` | Add the selected package |
| `Up` / `Down` | Navigate results |
| `Esc` | Close search |

## Init Wizard

| Key | Action |
|-----|--------|
| Type | Edit current field |
| `Backspace` | Delete character |
| `Enter` | Confirm and advance to next step |
| `Esc` | Go back to previous step |
| `Tab` | Cycle option (on Template step) |
| `Ctrl+C` | Cancel wizard |
