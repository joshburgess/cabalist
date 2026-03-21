# Editor Setup Guide

Cabalist includes `cabalist-lsp`, a Language Server Protocol server that brings `.cabal` file intelligence to any editor that supports LSP.

## Prerequisites

Install the `cabalist-lsp` binary:

```sh
cargo install --path crates/cabalist-lsp
# or download from GitHub Releases
```

Verify it's on your PATH:

```sh
cabalist-lsp --help
```

## LSP Features

`cabalist-lsp` provides these capabilities:

| Feature | What It Does |
|---------|-------------|
| **Diagnostics** | 16 opinionated lints shown as editor warnings/errors/hints |
| **Completions** | Hackage package names, GHC extensions, warning flags, field names, licenses, languages |
| **Hover** | Documentation for fields, extensions, warning flags, and packages (with Hackage synopsis) |
| **Code Actions** | Quick fixes for all lints (add bounds, remove duplicates, insert fields, etc.) |
| **Document Symbols** | Outline view showing metadata, components, common stanzas, and flags |
| **Formatting** | Round-trip safe formatting with optional dependency/module sorting |
| **Semantic Tokens** | Rich syntax highlighting for section keywords, field names, and comments |
| **Inlay Hints** | Latest Hackage version shown inline next to each dependency |
| **Go to Definition** | Jump from `import: stanza-name` to the `common stanza-name` definition |
| **Rename** | Rename a common stanza and automatically update all `import:` references |

## VS Code

### Quick Setup

1. Open the `editors/vscode/` directory:
   ```sh
   cd editors/vscode
   npm install
   npm run compile
   ```

2. In VS Code, run "Developer: Install Extension from Location..." and point to `editors/vscode/`.

### Configuration

Open VS Code settings and search for "cabalist":

| Setting | Default | Description |
|---------|---------|-------------|
| `cabalist.serverPath` | `""` (search PATH) | Path to `cabalist-lsp` binary |
| `cabalist.serverArgs` | `[]` | Additional arguments for `cabalist-lsp` |
| `cabalist.enableLsp` | `true` | Enable/disable the LSP |
| `cabalist.trace.server` | `off` | Trace LSP communication (`off`, `messages`, `verbose`) |

### Commands

- **Cabalist: Restart Language Server** — restart after config changes or crashes
- **Cabalist: Show Output Channel** — view server logs

### Status Bar

The status bar shows the LSP status:
- `$(loading~spin) Cabalist` — starting
- `$(check) Cabalist` — running
- `$(circle-slash) Cabalist` — stopped
- `$(error) Cabalist` — failed to start

Click the status bar item to open the output channel.

### Included Features

- **Syntax highlighting** — TextMate grammar for `.cabal` and `cabal.project` files
- **Snippets** — 10 snippets for common patterns (type `library`, `executable`, `common`, `dep`, `cabal-header`, etc.)
- **Folding** — Sections fold at their headers
- **Bracket matching** — Parentheses and quotes auto-close

### Troubleshooting

**"Failed to start language server"**: Ensure `cabalist-lsp` is on your PATH or set `cabalist.serverPath`.

**No completions**: The Hackage index may not be cached yet. Run `cabalist-cli update-index` or press `Ctrl+U` in the TUI.

**Diagnostics not updating**: Try "Cabalist: Restart Language Server" from the command palette.

---

## Neovim

### Option 1: nvim-lspconfig (Recommended)

Add to your Neovim configuration (e.g., `~/.config/nvim/init.lua`):

```lua
local lspconfig = require("lspconfig")
local configs = require("lspconfig.configs")

if not configs.cabalist then
  configs.cabalist = {
    default_config = {
      cmd = { "cabalist-lsp", "--stdio" },
      filetypes = { "cabal" },
      root_dir = lspconfig.util.root_pattern("*.cabal", "cabal.project"),
      single_file_support = true,
    },
  }
end

lspconfig.cabalist.setup({
  on_attach = function(client, bufnr)
    local opts = { buffer = bufnr, noremap = true, silent = true }
    vim.keymap.set("n", "gd", vim.lsp.buf.definition, opts)
    vim.keymap.set("n", "K", vim.lsp.buf.hover, opts)
    vim.keymap.set("n", "<leader>ca", vim.lsp.buf.code_action, opts)
    vim.keymap.set("n", "<leader>rn", vim.lsp.buf.rename, opts)
  end,
})
```

### Option 2: Native vim.lsp.start (Neovim 0.10+)

No plugins required. Add to `~/.config/nvim/ftplugin/cabal.lua`:

```lua
vim.lsp.start({
  name = "cabalist",
  cmd = { "cabalist-lsp", "--stdio" },
  root_dir = vim.fs.dirname(
    vim.fs.find({ "*.cabal", "cabal.project" }, { upward = true })[1]
  ),
})
```

### Filetype Detection

Neovim recognizes `.cabal` files automatically. For `cabal.project` files, add:

```lua
vim.filetype.add({
  filename = {
    ["cabal.project"] = "cabal",
    ["cabal.project.local"] = "cabal",
    ["cabal.project.freeze"] = "cabal",
  },
})
```

### Verifying

Open a `.cabal` file and check `:LspInfo` — you should see `cabalist` attached.

---

## Emacs

### Option 1: eglot (Built-in since Emacs 29)

Add to your init file:

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs
               '(haskell-cabal-mode . ("cabalist-lsp" "--stdio"))))

;; Auto-start on .cabal files:
(add-hook 'haskell-cabal-mode-hook #'eglot-ensure)
```

### Option 2: lsp-mode

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("cabalist-lsp" "--stdio"))
    :activation-fn (lsp-activate-on "cabal")
    :major-modes '(haskell-cabal-mode)
    :server-id 'cabalist-lsp
    :priority -1))

  (add-to-list 'lsp-language-id-configuration
               '(haskell-cabal-mode . "cabal")))

;; Auto-start on .cabal files:
(add-hook 'haskell-cabal-mode-hook #'lsp-deferred)
```

### Filetype Associations

Ensure `.cabal` files use `haskell-cabal-mode` (provided by `haskell-mode`):

```elisp
(add-to-list 'auto-mode-alist '("\\.cabal\\'" . haskell-cabal-mode))
(add-to-list 'auto-mode-alist '("cabal\\.project\\'" . haskell-cabal-mode))
```

### Prerequisites

- `haskell-mode` package (for `haskell-cabal-mode`)
- `cabalist-lsp` on your PATH

---

## Other Editors

Any editor that supports LSP can use `cabalist-lsp`. The server communicates over stdio:

```sh
cabalist-lsp --stdio
```

Configure your editor's LSP client with:
- **Command**: `cabalist-lsp --stdio`
- **Language ID**: `cabal`
- **File patterns**: `*.cabal`
- **Root markers**: `*.cabal`, `cabal.project`

## Logging

Set `RUST_LOG` to control log verbosity:

```sh
RUST_LOG=debug cabalist-lsp --stdio    # Verbose logging to stderr
RUST_LOG=info cabalist-lsp --stdio     # Default
RUST_LOG=warn cabalist-lsp --stdio     # Quiet
```

Logs are written to stderr so they don't interfere with the LSP stdio transport.
