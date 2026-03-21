;;; cabalist-lsp.el --- Cabalist LSP integration for Emacs  -*- lexical-binding: t; -*-

;; Cabalist LSP configuration for Emacs.
;;
;; Supports two LSP clients:
;;   1. eglot (built-in since Emacs 29)
;;   2. lsp-mode
;;
;; Prerequisites:
;;   - Install cabalist-lsp: `cargo install --path crates/cabalist-lsp`
;;   - Ensure `cabalist-lsp` is on your PATH
;;   - Install haskell-mode or cabal-mode for .cabal file major mode

;; ============================================================================
;; Option 1: eglot (recommended, built-in since Emacs 29)
;; ============================================================================

(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs
               '(haskell-cabal-mode . ("cabalist-lsp" "--stdio"))))

;; To auto-start eglot when opening .cabal files:
;; (add-hook 'haskell-cabal-mode-hook #'eglot-ensure)

;; ============================================================================
;; Option 2: lsp-mode
;; ============================================================================

(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("cabalist-lsp" "--stdio"))
    :activation-fn (lsp-activate-on "cabal")
    :major-modes '(haskell-cabal-mode)
    :server-id 'cabalist-lsp
    :priority -1))

  (add-to-list 'lsp-language-id-configuration '(haskell-cabal-mode . "cabal")))

;; To auto-start lsp-mode when opening .cabal files:
;; (add-hook 'haskell-cabal-mode-hook #'lsp-deferred)

;; ============================================================================
;; Filetype association (if needed)
;; ============================================================================

;; Ensure .cabal files use haskell-cabal-mode (provided by haskell-mode):
(add-to-list 'auto-mode-alist '("\\.cabal\\'" . haskell-cabal-mode))
(add-to-list 'auto-mode-alist '("cabal\\.project\\'" . haskell-cabal-mode))
(add-to-list 'auto-mode-alist '("cabal\\.project\\.local\\'" . haskell-cabal-mode))
(add-to-list 'auto-mode-alist '("cabal\\.project\\.freeze\\'" . haskell-cabal-mode))

(provide 'cabalist-lsp)
;;; cabalist-lsp.el ends here
