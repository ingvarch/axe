# Axe IDE -- Configuration Guide

This document covers all configuration options for Axe, including editor settings,
panel layout, theme customization, and how configuration files are resolved.

---

## Table of Contents

- [Config File Locations](#config-file-locations)
- [Configuration Sections](#configuration-sections)
  - [Editor](#editor)
  - [Tree](#tree)
  - [Terminal](#terminal)
  - [UI](#ui)
  - [AI](#ai)
- [Keybindings](#keybindings)
- [Config Merging](#config-merging)
- [Theme System](#theme-system)
  - [Built-in Themes](#built-in-themes)
  - [Custom Themes](#custom-themes)
  - [Theme File Format](#theme-file-format)
  - [Syntax Token Names](#syntax-token-names)
  - [Custom Theme Example](#custom-theme-example)

---

## Config File Locations

Axe reads configuration from TOML files at two levels:

| Level   | Path                              | Purpose                          |
|---------|-----------------------------------|----------------------------------|
| Global  | `~/.config/axe/config.toml`       | User-wide defaults               |
| Project | `.axe/config.toml` (project root) | Per-project overrides            |

On macOS and Linux, the global config directory follows the XDG convention and
lives under `~/.config/axe/`. Create the directory and file if they do not exist:

```bash
mkdir -p ~/.config/axe
touch ~/.config/axe/config.toml
```

For project-level overrides, create a `.axe/` directory at the root of your
project:

```bash
mkdir -p .axe
touch .axe/config.toml
```

You only need to specify the values you want to change. Any key not present in a
config file falls back to the default.

---

## Configuration Sections

Below is the full reference for every configuration section and key, shown with
default values.

### Editor

Controls text editing behavior.

```toml
[editor]
tab_size = 4                   # Number of spaces per tab stop (1-16)
insert_spaces = true           # Use spaces instead of tab characters
highlight_current_line = true  # Highlight the line under the cursor
scroll_margin = 5              # Lines of context kept above/below the cursor when scrolling
auto_save = false              # Automatically save modified buffers
auto_save_delay_ms = 1000      # Delay in milliseconds before auto-save triggers
word_wrap = false              # Wrap long lines (reserved for future use)
format_on_save = false         # Format buffer on save (reserved for future use)
```

**Key details:**

- `tab_size` must be between 1 and 16 inclusive. Values outside this range are
  clamped to the nearest bound.
- `insert_spaces` determines whether pressing Tab inserts space characters or a
  literal tab character. This does not convert existing tabs in a file.
- `scroll_margin` keeps the cursor away from the very top or bottom of the
  viewport, providing visible context around the editing position.
- `auto_save_delay_ms` only takes effect when `auto_save` is set to `true`. The
  timer resets on each edit, so the save fires after a pause in typing.

### Tree

Controls the file tree panel on the left side of the editor.

```toml
[tree]
visible = true                    # Show the file tree panel on startup
width = 25                        # Panel width in terminal columns
show_hidden = false               # Display hidden/ignored files
show_icons = true                 # Display file type icons (requires a Nerd Font)
sort_order = "directories_first"  # Sort order: "directories_first", "alphabetical", "type"
```

**Key details:**

- `width` is specified in terminal columns. The panel can also be resized
  interactively at runtime.
- `show_icons` requires a Nerd Font installed and configured in your terminal
  emulator. If icons render as missing-glyph boxes, either install a Nerd Font
  or set this to `false`.

### Terminal

Controls the embedded terminal panel at the bottom of the editor.

```toml
[terminal]
visible = true          # Show the terminal panel on startup
height_percent = 30     # Terminal height as a percentage of the window (10-90)
shell = ""              # Shell executable path; empty string means auto-detect
scrollback_lines = 10000 # Number of scrollback lines retained
```

**Key details:**

- `height_percent` must be between 10 and 90 inclusive. This prevents the
  terminal from consuming the entire window or becoming too small to use.
- When `shell` is an empty string, Axe detects the user's shell from the `SHELL`
  environment variable. Set an explicit path (for example, `"/opt/homebrew/bin/fish"`)
  to override this behavior.
- `scrollback_lines` controls how many lines of terminal output are kept in
  memory. Higher values use more memory.

### UI

Controls the overall visual appearance.

```toml
[ui]
theme = "axe-dark"       # Name of the active theme
border_style = "rounded" # Panel border style: "rounded" or "plain"
```

**Key details:**

- `theme` refers to a theme by name. Built-in themes are resolved automatically.
  Custom themes are loaded from the themes directory (see the Theme System section
  below).
- `border_style` accepts two values:
  - `"rounded"` -- uses rounded Unicode box-drawing characters.
  - `"plain"` -- uses straight-line box-drawing characters.

### AI

Controls the toggleable AI chat overlay (default hotkey: `Ctrl+Shift+A`).

```toml
[ai]
# Agent ID to spawn when the overlay is opened. If omitted, Axe runs the
# first-run picker on first use and writes the chosen ID back here.
default = "claude"

# Optional: override a built-in agent or register a custom one.
# Key is the agent ID; `command`/`args` are how it is launched in the PTY;
# `display_name` is the label shown in the picker and overlay title.
[ai.agents.my-agent]
command = "/opt/bin/my-agent"
args = ["--experimental"]
display_name = "My Custom Agent"
```

**Key details:**

- Axe ships with seven built-in agent IDs: `claude`, `codex`, `gemini`, `qwen`,
  `aider`, `opencode`, `goose`. Each resolves its `command` field against
  `$PATH`, so you usually don't need to touch `[ai.agents.*]` at all -- just
  install the CLI and pick it from the first-run list.
- Entries in `[ai.agents.<id>]` that match a built-in ID override the built-in;
  entries with new IDs are appended to the picker.
- Axe writes the `default` key back through `toml_edit` whenever you pick an
  agent via the picker. Existing comments and unrelated sections are preserved
  -- your hand-authored config survives the round-trip.
- The AI section always lives in the global config (`~/.config/axe/config.toml`)
  because the choice of default agent is a per-user preference, not a
  per-project one.
- Hiding the overlay with the toggle hotkey does not kill the PTY: the session
  survives until you either quit Axe, run `AI: Kill Current Session` from the
  command palette, or the agent exits on its own.

---

## Keybindings

All keybindings can be customized in the `[keybindings]` section. Each entry
maps a key combination to a command name. User-defined bindings override the
built-in defaults.

```toml
[keybindings]
"ctrl+q" = "request_quit"
"ctrl+s" = "save"
"ctrl+b" = "toggle_tree"
"ctrl+t" = "toggle_terminal"
"alt+]" = "next_buffer"
"alt+[" = "prev_buffer"
```

### Key Combination Format

Key combos are written as modifier+key, joined by `+`:

- **Modifiers:** `ctrl`, `alt`, `shift` (case-insensitive, combinable)
- **Special keys:** `esc`, `enter`, `tab`, `backtab`, `space`, `backspace`,
  `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`,
  `pagedown`, `f1`-`f12`
- **Character keys:** any single character (`q`, `a`, `]`, `/`, etc.)

Examples: `"ctrl+shift+z"`, `"alt+]"`, `"f12"`, `"esc"`

### Available Commands

| Command Name                 | Description                          |
|------------------------------|--------------------------------------|
| `request_quit`               | Quit (with confirmation if unsaved)  |
| `save`                       | Save current buffer                  |
| `toggle_tree`                | Show/hide file tree panel            |
| `toggle_terminal`            | Show/hide terminal panel             |
| `focus_next`                 | Cycle focus to next panel            |
| `focus_prev`                 | Cycle focus to previous panel        |
| `focus_tree`                 | Focus file tree                      |
| `focus_editor`               | Focus editor                         |
| `focus_terminal`             | Focus terminal                       |
| `show_help`                  | Toggle help overlay                  |
| `close_overlay`              | Close any open overlay               |
| `enter_resize_mode`          | Enter panel resize mode              |
| `equalize_layout`            | Reset panel sizes to defaults        |
| `zoom_panel`                 | Zoom focused panel to full screen    |
| `undo`                       | Undo last edit                       |
| `redo`                       | Redo last edit                       |
| `copy`                       | Copy selection to clipboard          |
| `cut`                        | Cut selection to clipboard           |
| `paste`                      | Paste from clipboard                 |
| `select_all`                 | Select all text in buffer            |
| `find`                       | Open search bar                      |
| `close_buffer`               | Close current buffer                 |
| `next_buffer`                | Switch to next buffer                |
| `prev_buffer`                | Switch to previous buffer            |
| `new_terminal_tab`           | Open new terminal tab                |
| `close_terminal_tab`         | Close active terminal tab            |
| `toggle_icons`               | Toggle file tree icons               |
| `toggle_ignored`             | Toggle hidden/ignored files          |
| `scroll_terminal_up`         | Scroll terminal up one page          |
| `scroll_terminal_down`       | Scroll terminal down one page        |
| `scroll_terminal_top`        | Scroll terminal to top               |
| `scroll_terminal_bottom`     | Scroll terminal to bottom            |
| `activate_terminal_tab:N`    | Switch to terminal tab N (0-based)   |
| `toggle_ai_overlay`          | Show/hide the AI chat overlay        |
| `select_ai_agent`            | Open the AI agent picker             |
| `kill_ai_session`            | Kill the current AI chat session     |

---

## Config Merging

Configuration values are resolved in three layers, where each layer overrides
the previous one:

```
1. Built-in defaults (hardcoded in the application)
      |
      v
2. Global config (~/.config/axe/config.toml)
      |
      v
3. Project config (.axe/config.toml in the project root)
```

Merging is performed per key, not per section. This means you can override a
single key in the `[editor]` section at the project level without needing to
repeat the entire section.

**Example:** If your global config sets `tab_size = 4` and a project config sets
`tab_size = 2`, the editor uses `tab_size = 2` for that project while all other
editor settings remain at their global (or default) values.

---

## Theme System

Axe uses a TOML-based theme system for full control over UI and syntax colors.

### Built-in Themes

Axe ships with two built-in themes:

| Name         | Description                          |
|--------------|--------------------------------------|
| `axe-dark`   | Default theme based on One Dark      |
| `axe-light`  | Light theme based on One Light       |

The default theme is `axe-dark`. Change the active theme in your config:

```toml
[ui]
theme = "axe-light"
```

### Custom Themes

To create a custom theme:

1. Create the themes directory if it does not exist:

   ```bash
   mkdir -p ~/.config/axe/themes
   ```

2. Create a TOML file named after your theme:

   ```bash
   touch ~/.config/axe/themes/my-theme.toml
   ```

3. Reference it by name (the filename without the `.toml` extension):

   ```toml
   [ui]
   theme = "my-theme"
   ```

### Theme File Format

A theme file contains five sections. All color values are specified as
`"#rrggbb"` hex strings.

#### [base]

Core background and foreground colors used across the entire interface.

```toml
[base]
background = "#282c34"
foreground = "#abb2bf"
```

#### [ui]

Colors for panels, borders, overlays, and the status bar.

```toml
[ui]
panel_border = "#4c5263"
panel_border_active = "#61afef"
status_bar_bg = "#21252b"
status_bar_fg = "#abb2bf"
status_bar_key = "#828997"
overlay_border = "#61afef"
overlay_bg = "#282c34"
resize_border = "#e5c07b"
tree_selection_bg = "#323741"
tab_bar_bg = "#21252b"
tab_active_bg = "#282c34"
tab_active_fg = "#abb2bf"
tab_inactive_fg = "#828997"
```

#### [gutter]

Colors for the line number gutter.

```toml
[gutter]
background = "#23272e"
line_number = "#4c5263"
line_number_active = "#abb2bf"
```

#### [editor]

Colors for editor-specific highlights such as the cursor line, selections, and
search matches.

```toml
[editor]
cursor_line_bg = "#2d323c"
selection_bg = "#434c5e"
search_match_bg = "#3c3c1e"
search_active_match_bg = "#e5c07b"
search_active_match_fg = "#282c34"
```

#### [syntax]

Colors for syntax highlighting tokens. Each token supports the following
properties:

| Property | Type     | Required | Description                   |
|----------|----------|----------|-------------------------------|
| `fg`     | `string` | Yes      | Foreground color (`"#rrggbb"`) |
| `bg`     | `string` | No       | Background color (`"#rrggbb"`) |
| `bold`   | `bool`   | No       | Render text in bold            |
| `italic` | `bool`   | No       | Render text in italic          |

```toml
[syntax]
keyword   = { fg = "#c678dd", bold = true }
string    = { fg = "#98c379" }
comment   = { fg = "#5c6370", italic = true }
function  = { fg = "#61afef" }
type      = { fg = "#e5c07b" }
variable  = { fg = "#e06c75" }
constant  = { fg = "#d19a66", bold = true }
number    = { fg = "#d19a66" }
operator  = { fg = "#56b6c2" }
punctuation = { fg = "#abb2bf" }
property  = { fg = "#e06c75" }
attribute = { fg = "#e5c07b" }
tag       = { fg = "#e06c75" }
escape    = { fg = "#56b6c2" }
builtin   = { fg = "#56b6c2" }
```

### Syntax Token Names

The following syntax token names are recognized by the highlighting engine:

| Token         | Typical usage                                      |
|---------------|----------------------------------------------------|
| `keyword`     | Language keywords (`if`, `fn`, `let`, `return`)     |
| `string`      | String literals and string interpolation            |
| `comment`     | Line and block comments                             |
| `function`    | Function and method names                           |
| `type`        | Type names, classes, structs, interfaces            |
| `variable`    | Variable and parameter names                        |
| `constant`    | Constants and enum variants                         |
| `number`      | Numeric literals (integers, floats)                 |
| `operator`    | Operators (`+`, `-`, `=`, `=>`, `->`)               |
| `punctuation` | Brackets, braces, commas, semicolons                |
| `property`    | Struct fields, object properties                    |
| `attribute`   | Annotations, decorators, derive macros              |
| `tag`         | HTML/XML/JSX tags                                   |
| `escape`      | Escape sequences in strings (`\n`, `\t`)            |
| `builtin`     | Built-in functions and variables (`println!`, `len`)|

### Custom Theme Example

Below is a minimal custom theme file demonstrating the required structure. Save
it as `~/.config/axe/themes/nord.toml` and activate it with `theme = "nord"` in
your config.

```toml
# Nord-inspired theme for Axe

[base]
background = "#2e3440"
foreground = "#d8dee9"

[ui]
panel_border = "#3b4252"
panel_border_active = "#88c0d0"
status_bar_bg = "#2e3440"
status_bar_fg = "#d8dee9"
status_bar_key = "#4c566a"
overlay_border = "#88c0d0"
overlay_bg = "#2e3440"
resize_border = "#ebcb8b"
tree_selection_bg = "#3b4252"
tab_bar_bg = "#2e3440"
tab_active_bg = "#3b4252"
tab_active_fg = "#eceff4"
tab_inactive_fg = "#4c566a"

[gutter]
background = "#2e3440"
line_number = "#4c566a"
line_number_active = "#d8dee9"

[editor]
cursor_line_bg = "#3b4252"
selection_bg = "#434c5e"
search_match_bg = "#4c566a"
search_active_match_bg = "#ebcb8b"
search_active_match_fg = "#2e3440"

[syntax]
keyword     = { fg = "#81a1c1", bold = true }
string      = { fg = "#a3be8c" }
comment     = { fg = "#616e88", italic = true }
function    = { fg = "#88c0d0" }
type        = { fg = "#ebcb8b" }
variable    = { fg = "#d8dee9" }
constant    = { fg = "#b48ead", bold = true }
number      = { fg = "#b48ead" }
operator    = { fg = "#81a1c1" }
punctuation = { fg = "#eceff4" }
property    = { fg = "#d8dee9" }
attribute   = { fg = "#ebcb8b" }
tag         = { fg = "#bf616a" }
escape      = { fg = "#ebcb8b" }
builtin     = { fg = "#88c0d0" }
```
