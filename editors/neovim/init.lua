-- Cabalist LSP configuration for Neovim
--
-- Option 1: Using nvim-lspconfig (recommended)
-- Add this to your Neovim configuration after installing nvim-lspconfig.
--
-- Option 2: Manual setup (see below for native vim.lsp.start approach)

-- ============================================================================
-- Option 1: nvim-lspconfig custom server setup
-- ============================================================================

-- Until cabalist-lsp is added to nvim-lspconfig's built-in configs,
-- register it as a custom server:

local lspconfig = require("lspconfig")
local configs = require("lspconfig.configs")

if not configs.cabalist then
  configs.cabalist = {
    default_config = {
      cmd = { "cabalist-lsp", "--stdio" },
      filetypes = { "cabal" },
      root_dir = lspconfig.util.root_pattern("*.cabal", "cabal.project"),
      single_file_support = true,
      settings = {},
    },
    docs = {
      description = [[
cabalist-lsp — Language server for Haskell .cabal files.

Provides diagnostics (opinionated lints), completions (Hackage packages,
GHC extensions, field names), hover documentation, and quick-fix code actions.

Install: `cargo install --path crates/cabalist-lsp`
Homepage: https://github.com/joshburgess/cabalist
      ]],
    },
  }
end

lspconfig.cabalist.setup({
  -- Override the server path if cabalist-lsp is not on your PATH:
  -- cmd = { "/path/to/cabalist-lsp", "--stdio" },

  on_attach = function(client, bufnr)
    -- Optional: set up keymaps for LSP actions
    local opts = { buffer = bufnr, noremap = true, silent = true }
    vim.keymap.set("n", "gd", vim.lsp.buf.definition, opts)
    vim.keymap.set("n", "K", vim.lsp.buf.hover, opts)
    vim.keymap.set("n", "<leader>ca", vim.lsp.buf.code_action, opts)
    vim.keymap.set("n", "<leader>rn", vim.lsp.buf.rename, opts)
  end,
})

-- ============================================================================
-- Option 2: Native vim.lsp.start (Neovim 0.10+, no plugins required)
-- ============================================================================
--
-- Add this to ~/.config/nvim/ftplugin/cabal.lua:
--
--   vim.lsp.start({
--     name = "cabalist",
--     cmd = { "cabalist-lsp", "--stdio" },
--     root_dir = vim.fs.dirname(vim.fs.find({ "*.cabal", "cabal.project" }, { upward = true })[1]),
--   })

-- ============================================================================
-- Filetype detection
-- ============================================================================
--
-- Neovim recognises .cabal files out of the box. If yours doesn't, add:
--
--   vim.filetype.add({
--     extension = { cabal = "cabal" },
--     filename = {
--       ["cabal.project"] = "cabal",
--       ["cabal.project.local"] = "cabal",
--       ["cabal.project.freeze"] = "cabal",
--     },
--   })
