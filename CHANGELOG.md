# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Derive version from git tags with nightly and dev variants

### Changed

- Unify CI checks via make check as single source of truth

### Fixed

- Handle shallow clones without tags in version detection
- Apply cargo fmt to build.rs

## [0.1.0] - 2026-04-07

### Added

- Cargo workspace with 7 crate stubs (axe-core, axe-editor, axe-tree, axe-terminal, axe-lsp, axe-ui, axe-config)
- TUI event loop with raw terminal and clap CLI
- Three-panel layout with theme and status bar
- Focus system with panel switching and visual feedback
- Command system, keymap resolver, panel toggles, and help overlay
- Panel resize mode with Ctrl+R, arrow keys, and visual feedback
- Mouse drag resize and click-to-focus for panels
- Zoom panel toggle with Ctrl+Z, status indicator, and zoomed title suffix
- File tree directory listing with sorted display and CLI path arg
- Keyboard navigation, expand/collapse, selection highlight, and scrolling in tree
- Gitignore filtering, show dot-files, and default focus to Files panel
- File create, delete, and rename operations with inline input in tree
- File type icons with Nerd Font glyphs and Ctrl+I toggle
- Mouse click support for file tree selection and open/toggle
- Single-click preview and double-click permanent open in tree
- Preserve expanded folder state on refresh and filesystem watcher
- Mouse wheel and keyboard scrolling in Files panel with horizontal scroll clamping
- File opening from tree with content display, line numbers, and status bar
- Cursor movement with viewport scrolling, line highlight, and status bar position
- Text insertion, deletion, save and autosave with modified indicator
- Undo/redo with time-based edit grouping
- Text selection, clipboard ops, mouse selection and status bar notifications
- In-file search with Ctrl+F, match navigation, case/regex toggles
- Multiple buffers with tab bar, Alt+]/[ switching, close confirmation
- Tree-sitter syntax highlighting for 14 languages with incremental parsing
- Terraform and HCL language support with tree-sitter highlighting and LSP integration
- Find and Replace with Ctrl+H keybinding
- Go to Line dialog with Ctrl+G keybinding
- Editor scrollbar with mouse click and drag support
- Mouse wheel scrolling in editor panel with horizontal trackpad support
- Shell spawning in PTY with ANSI color rendering
- Keyboard input forwarding to PTY with escape sequence translation
- Shell spawning in project working directory
- Quit confirmation dialog, unbind Tab and Ctrl+C for terminal passthrough
- Multiple terminal tabs with Alt+1-9 switching, mouse clicks, smart path titles, and auto-close on shell exit
- Scrollback buffer with keyboard/mouse scroll, scrollbar, and SGR mouse fix
- Mouse text selection with clipboard copy in terminal
- Confirmation dialog when closing terminal tab with running process
- LSP client infrastructure with JSON-RPC transport and multi-server manager
- LSP diagnostics with gutter icons, underlines, and status bar
- Code completion with popup, filtering, and auto-trigger
- Go To Definition (F12) and Find References (Shift+F12) with location list overlay
- Hover information with tooltip rendering and mouse hover delay
- Format document and format-on-save via textDocument/formatting
- Fuzzy file finder overlay with Ctrl+P
- Command palette overlay with Ctrl+Shift+P and F1
- Project-wide search overlay with Ctrl+Shift+F and F2
- Multi-click text selection for editor and terminal panels
- Unified navigable modal to replace y/N confirmation dialogs
- Git gutter diff indicators and highlight modified files in tree panel
- Current branch name in status bar with periodic refresh
- Status bar with three-section layout and line ending detection
- Startup welcome screen with ASCII logo, version, and shortcuts
- Session save and restore with --no-session flag
- 'No files open' message in empty editor panel including zoomed state
- Configuration system with theme engine, configurable keybindings, and per-project overrides
- Help overlay with multi-column layout and section separators
- Multi-arch CI release with version string and git hash
- GitHub Actions workflow with test, lint, and build jobs
- Nightly build workflow with GitHub Release
- README with features, keybindings, LSP setup, and configuration
- Screenshot in README

### Changed

- Replace Ctrl+1/2/3 with Alt+1/2/3 panel focus and forward Shift+Tab to PTY
- Unify tab hotkeys across Editor and Terminal panels with focus-based dispatch
- Replace Alt-based keybindings with Ctrl+Shift for cross-terminal compatibility
- Rebind Help to F1, Find and Replace to Ctrl+R, Resize to Ctrl+N
- Use global gitignore instead of per-project .axe/.gitignore
- Use ~/.config/axe/ instead of platform-specific config_dir
- Upgrade ratatui to 0.30 and add synchronized output to prevent scroll tearing
- Update crossterm to 0.29, tree-sitter to 0.25, and all grammar crates to latest
- Upgrade actions/checkout to v6 and remove redundant check job
- Upgrade actions/upload-artifact to v5 and then v7 to fix Node.js 20 deprecation
- Split oversized lib.rs and buffer.rs into modular subfiles
- Split monolithic app.rs into modular app/ directory
- Extract tree line builders and add impact analysis comments

### Fixed

- Use vendored OpenSSL for cross-compilation support
- Configure git user in tests for CI compatibility
- Resolve clippy warnings and formatting issues in CI
- Replace shared temp_dir with isolated tempfile in flaky tree tests
- Expand tab characters to spaces in editor rendering and fix syntax highlight priority
- Check right boundary in screen_to_tree_node_index to prevent editor clicks triggering tree actions
- Use stored terminal tab bar area for accurate mouse click detection
- Forward DSR cursor position responses back to PTY to prevent crossterm timeout
- Hide terminal panel when closing last terminal tab via Alt+W
- Prevent set_viewport_height from undoing mouse wheel scroll in tree
- Eliminate ghost characters and rendering artifacts after resize and commands in terminal
