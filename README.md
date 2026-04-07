```
 @@@@@@   @@@  @@@  @@@@@@@@
@@@@@@@@  @@@  @@@  @@@@@@@@
@@!  @@@  @@!  !@@  @@!
!@!  @!@  !@!  @!!  !@!
@!@!@!@!   !@@!@!   @!!!:!
!!!@!!!!    @!!!    !!!!!:
!!:  !!!   !: :!!   !!:
:!:  !:!  :!:  !:!  :!:
::   :::   ::  :::   :: ::::
 :   : :   :   ::   : :: ::
```

A terminal-based IDE written in Rust. Fast, lightweight, keyboard-driven.

**This is a fun project built with 100% vibe coding.**

## Features

- **Three-panel layout** -- file tree, code editor, integrated terminal
- **Syntax highlighting** -- tree-sitter based, 13+ languages out of the box
- **LSP support** -- diagnostics, completions, go-to-definition, hover
- **Fuzzy file finder** -- Ctrl+P, powered by nucleo
- **Command palette** -- Ctrl+Shift+P, search and execute any command
- **Project-wide search** -- Ctrl+Shift+F with regex, case sensitivity, include/exclude filters
- **Multiple editor tabs** -- open and switch between files
- **Multiple terminal tabs** -- run several shells side by side
- **Configurable** -- TOML-based config, custom keybindings, themes
- **Mouse support** -- click, drag panel borders, select text

## Installation

Requires Rust 1.75+.

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/axe .
```

## Usage

```bash
axe              # Open current directory
axe /path/to/dir # Open specific directory
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+Q | Quit |
| Tab / Shift+Tab | Cycle focus between panels |
| Alt+1 / Alt+2 / Alt+3 | Focus Files / Editor / Terminal |
| Ctrl+B | Toggle file tree |
| Ctrl+\` | Toggle terminal |
| Ctrl+Z | Zoom active panel |
| Ctrl+P | Fuzzy file finder |
| Ctrl+Shift+P | Command palette |
| Ctrl+Shift+F | Project-wide search |
| Ctrl+F | Search in file |
| Ctrl+S | Save file |
| Ctrl+W | Close buffer |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous buffer |
| Ctrl+Shift+T | New terminal tab |
| Ctrl+R | Enter resize mode |

## LSP (Language Server Protocol)

Axe includes built-in LSP support. Language servers are started automatically when you open a file with a recognized extension. No extra configuration is required if the server binary is in your PATH.

### Supported Languages

| Language | Server | Install |
|----------|--------|---------|
| Rust | rust-analyzer | `rustup component add rust-analyzer` |
| Go | gopls | `go install golang.org/x/tools/gopls@latest` |
| Python | pyright | `npm i -g pyright` |
| TypeScript / JavaScript | typescript-language-server | `npm i -g typescript-language-server typescript` |
| C / C++ | clangd | `brew install llvm` or install via Xcode |
| Lua | lua-language-server | `brew install lua-language-server` |
| TOML | taplo | `cargo install taplo-cli` |
| Shell (Bash/Zsh) | bash-language-server | `npm i -g bash-language-server` |

### Custom LSP Configuration

Override or add servers in your config file (`~/.config/axe/config.toml`):

```toml
[lsp.rust]
command = "rust-analyzer"

[lsp.python]
command = "pylsp"

[lsp.ruby]
command = "solargraph"
args = ["stdio"]
```

User-defined entries override the built-in defaults.

## Configuration

Axe loads configuration from two locations:

1. `~/.config/axe/config.toml` -- global settings
2. `<project>/.axe/config.toml` -- project-level overrides

Example:

```toml
[editor]
tab_size = 2
insert_spaces = true
auto_save = true
format_on_save = true

[tree]
show_hidden = false
show_icons = true

[terminal]
shell = "/bin/zsh"
scrollback_lines = 10000

[ui]
theme = "axe-dark"

[keybindings]
"ctrl+q" = "request_quit"
"alt+x" = "editor_save"
```

## Themes

Two built-in themes: `axe-dark` (default) and `axe-light`.

Custom themes can be placed in `~/.config/axe/themes/` as TOML files.

## Architecture

Axe is structured as a Cargo workspace with focused crates:

| Crate | Purpose |
|-------|---------|
| axe-core | Central state, events, commands, keymap |
| axe-editor | Code editor (rope, tree-sitter, cursor, undo) |
| axe-tree | File tree panel |
| axe-terminal | Embedded terminal (PTY, VT parsing) |
| axe-lsp | Language Server Protocol client |
| axe-ui | Rendering, layout, overlays, themes |
| axe-config | Configuration parsing |

## License

MIT
