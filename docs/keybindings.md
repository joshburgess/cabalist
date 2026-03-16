# Keybindings

## Global (available in all views)

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Ctrl+C` | Quit |
| `Ctrl+S` | Save `.cabal` file to disk |
| `Ctrl+R` | Reload `.cabal` file from disk |
| `Ctrl+Z` | Undo last edit |
| `?` | Show contextual help |
| `Esc` | Go back / close popup |
| `j` / `Down` | Move selection down |
| `k` / `Up` | Move selection up |
| `g` | Jump to top of list |
| `G` | Jump to bottom of list |
| `Enter` | Confirm / select current item |
| `Tab` | Switch to next component |
| `Shift+Tab` | Switch to previous component |
| `/` | Open search/filter |

## Dashboard

| Key | Action |
|-----|--------|
| `d` | Switch to Dependencies view |
| `e` | Switch to Extensions view |
| `b` | Switch to Build view |
| `m` | Switch to Metadata view |
| `i` | Start init wizard |

## Dependencies

| Key | Action |
|-----|--------|
| `a` | Add a dependency (opens search popup) |
| `r` | Remove the selected dependency |
| `Tab` | Switch component |

## Extensions

| Key | Action |
|-----|--------|
| `Space` | Toggle the selected extension on/off |
| `i` | Show info about the selected extension |
| `/` | Filter extensions by name |

## Build Output

| Key | Action |
|-----|--------|
| `b` | Start a build (`cabal build`) |
| `t` | Run tests (`cabal test`) |
| `c` | Clean (`cabal clean`) |
| `n` / `]` | Jump to next error/warning |
| `p` / `[` | Jump to previous error/warning |
| Mouse scroll | Scroll build output |

## Search Popup

| Key | Action |
|-----|--------|
| Type | Filter results |
| `Backspace` | Delete character |
| `Enter` | Select highlighted result |
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
