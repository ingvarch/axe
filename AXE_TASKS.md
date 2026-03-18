# Axe IDE — Implementation Tasks

This document contains an ordered list of implementation tasks for the Axe terminal IDE.
Each task builds on the previous one. Tasks are designed to be executed one at a time
by Claude Code or a developer. Each task should result in a **compiling, runnable** state.

Reference: See `AXE_ARCHITECTURE.md` for full architecture details.

---

## Phase 1: Skeleton & Core Loop

### Task 1.1 — Initialize Cargo Workspace [DONE]

Create the Cargo workspace with all crate stubs. Every crate should have a `lib.rs`
with a placeholder comment. The root should have a `src/main.rs` that prints "Axe IDE".

**Acceptance criteria:**
- [x] `cargo build` succeeds
- [x] `cargo run` prints "Axe IDE v0.1.0"
- [x] Workspace members: `axe-core`, `axe-editor`, `axe-tree`, `axe-terminal`, `axe-lsp`, `axe-ui`, `axe-config`
- [x] Each crate has `Cargo.toml` and `src/lib.rs`
- [x] Root `Cargo.toml` defines the workspace and the `axe` binary

**Key dependencies to add:**
- Root binary: `axe-core`, `axe-ui`, `axe-config`, `crossterm`, `ratatui`, `tokio`
- `axe-core`: `crossterm`, `serde`, `tokio`
- `axe-ui`: `ratatui`, `crossterm`, `axe-core`
- `axe-config`: `serde`, `toml`
- Other crates: just `axe-core` and `serde` for now (dependencies added as needed)

---

### Task 1.2 — Basic Event Loop with Raw Terminal [DONE]

Set up the main event loop: enter raw mode, create a Ratatui terminal, render an empty
screen, and handle `q` to quit. This is the foundation everything else builds on.

**Acceptance criteria:**
- [x] Running `cargo run` enters the TUI (alternate screen, raw mode)
- [x] Pressing `q` exits cleanly (terminal restored to normal)
- [x] An empty screen is rendered with a centered "Axe IDE" text
- [x] `Ctrl+C` also exits cleanly
- [x] Terminal is always restored on panic (install panic hook)

**Implementation details:**
- Use `crossterm` as the Ratatui backend
- Install a custom panic hook that restores the terminal before printing the panic
- Event loop reads crossterm events with a small timeout (e.g., 50ms)
- Use `tokio` with `#[tokio::main]` but the event loop itself is synchronous for now

---

### Task 1.3 — Three-Panel Layout (Static) [DONE]

Render three panels: file tree (left), editor (top-right), terminal (bottom-right).
No content yet — just colored borders with panel titles.

**Acceptance criteria:**
- [x] Three panels visible with rounded borders
- [x] Left panel: title "Files", takes ~20% width
- [x] Top-right panel: title "Editor", takes ~70% of remaining height
- [x] Bottom-right panel: title "Terminal", takes ~30% of remaining height
- [x] Bottom status bar: shows "Axe IDE v0.1.0 | Press q to quit"
- [x] Layout adjusts on terminal resize
- [x] Different border colors for active vs inactive panels (just visual, no focus logic yet)

**Implementation details:**
- Use `ratatui::layout::Layout` with `Constraint::Percentage`
- Use `Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)`
- Define a basic `Theme` struct in `axe-ui` with colors for borders, backgrounds, etc.
- Hardcode a dark theme for now (similar to One Dark)

---

### Task 1.4 — Focus System & Panel Switching [DONE]

Implement focus switching between the three panels. The active panel should have
a highlighted border.

**Acceptance criteria:**
- `Tab` cycles focus: Files → Editor → Terminal → Files → ...
- `Shift+Tab` cycles focus in reverse
- Active panel has a bright border (e.g., bright cyan), inactive panels have dim borders
- Active panel title is bold
- Status bar shows which panel is focused (e.g., "Focus: Editor")
- `Ctrl+1` focuses Files, `Ctrl+2` focuses Editor, `Ctrl+3` focuses Terminal

**Implementation details:**
- Add `FocusTarget` enum to `axe-core` (Tree, Editor, Terminal)
- Add `focus: FocusTarget` field to `AppState`
- Keypress handling: match on focus target to decide which panel processes the key

---

### Task 1.5 — Command System & Keybinding Infrastructure [DONE]

Replace hardcoded key checks with a proper command system. This is critical
infrastructure — all future features will use it.

**Acceptance criteria:**
- [x] `Command` enum exists in `axe-core` with variants: `Quit`, `FocusNext`, `FocusPrev`, `FocusTree`, `FocusEditor`, `FocusTerminal`, `ToggleTree`, `ToggleTerminal`
- [x] `KeymapResolver` maps `KeyEvent` → `Command`
- [x] Default keybindings loaded from code (not file yet)
- [x] `Ctrl+Q` quits (not just `q` anymore, since we'll need `q` for typing)
- [x] `Ctrl+B` toggles file tree visibility (panel disappears/reappears)
- [x] `` Ctrl+` `` toggles terminal visibility
- [x] All keybindings go through the command system, no more raw key checks in the event loop

**Implementation details:**
- `KeymapResolver` is a `HashMap<(KeyModifiers, KeyCode), Command>` for now
- Context-aware keymaps can be added later, start with global-only
- Event loop becomes: `key → resolve_command → dispatch_command → update_state`

---

## Phase 2: Panel Resize & Layout

### Task 2.1 — Panel Resize Mode [DONE]

Implement zellij-style panel resizing with a dedicated resize mode.

**Acceptance criteria:**
- [x] `Ctrl+R` enters resize mode
- [x] Status bar shows `-- RESIZE --` in a distinct color (e.g., yellow)
- [x] Focused panel border changes to yellow/highlighted color in resize mode
- [x] Arrow keys resize the active panel:
  - When Files is focused: `←`/`→` change tree width
  - When Editor is focused: `↑`/`↓` change editor/terminal height split
  - When Terminal is focused: `↑`/`↓` change editor/terminal height split
- [x] `Esc` or `Enter` exits resize mode
- [x] Minimum panel size: 10%, maximum: 90%
- [x] Resize step: 2% per arrow press
- [x] `=` equalizes all panels to default sizes (20% tree, 70/30 editor/terminal)

**Implementation details:**
- Add `ResizeModeState` to `AppState` (active: bool, step, min/max)
- Add resize commands to `Command` enum
- Add `ResizeMode` context to keymap (when active, arrow keys map to resize commands)
- Store `tree_width_pct` and `editor_height_pct` in `LayoutManager`

---

### Task 2.2 — Mouse Resize (Drag Panel Borders) [DONE]

Allow resizing panels by clicking and dragging borders.

**Acceptance criteria:**
- Clicking on the vertical border (between tree and editor/terminal) and dragging horizontally resizes the tree panel
- Clicking on the horizontal border (between editor and terminal) and dragging vertically resizes the split
- Cursor position near a border is detected (within 1 cell)
- Resize updates in real-time while dragging
- Same min/max constraints as keyboard resize

**Implementation details:**
- Enable mouse capture in crossterm
- Track mouse state: `None`, `DraggingVerticalBorder`, `DraggingHorizontalBorder`
- On `MouseEvent::Down`: check if near a border, start drag
- On `MouseEvent::Drag`: update panel percentages
- On `MouseEvent::Up`: end drag

---

### Task 2.3 — Zoom Panel Toggle [DONE]

Maximize the active panel to fill the entire screen.

**Acceptance criteria:**
- `Ctrl+Z` toggles zoom on the active panel
- When zoomed: only the active panel is visible, filling the entire area
- When un-zoomed: layout returns to previous state (preserving resize ratios)
- Status bar shows `[ZOOM]` indicator when zoomed
- Panel title shows `(zoomed)` suffix

**Implementation details:**
- Add `zoomed_panel: Option<FocusTarget>` to `LayoutManager`
- When zoomed, the render function skips the Layout split and gives 100% to the active panel

---

## Phase 3: File Tree

### Task 3.1 — File Tree: Display Directory Listing [DONE]

Show the current working directory's contents in the file tree panel.

**Acceptance criteria:**
- [x] On startup, the file tree shows the contents of the directory passed as argument (`axe .`) or CWD
- [x] Directories shown before files, both sorted alphabetically
- [x] Directories have a `▸` (collapsed) or `▾` (expanded) prefix
- [x] Files have a ` ` (space) prefix for alignment
- [x] Indentation shows nesting depth (2 spaces per level)
- [x] The project root name is shown as the first item (bold)
- [x] Hidden files (starting with `.`) are hidden by default

**Implementation details:**
- Implement `FileTree` struct in `axe-tree` with `Vec<TreeNode>`
- Read directory with `std::fs::read_dir`
- Lazy loading: only read top-level at startup
- Store `depth`, `expanded`, `kind` on each node

---

### Task 3.2 — File Tree: Navigation & Expand/Collapse ✅

Navigate the tree with keyboard and expand/collapse directories.

**Acceptance criteria:**
- [x] `↑`/`↓` moves selection up/down (highlighted row)
- [x] `Enter` or `→` on a directory: expands it (loads children)
- [x] `Enter` on a file: does nothing yet (will open in editor later)
- [x] `←` on an expanded directory: collapses it
- [x] `←` on a file or collapsed directory: moves to parent directory
- [x] Scrolling works when the tree is taller than the panel
- [x] `Home` jumps to the first item, `End` to the last
- [x] Selection wraps around (last item → first, first → last)

**Implementation details:**
- Added `viewport_height` to `FileTree` with scroll management
- Tree-focus key interception in `handle_key_event()` (after resize mode, before global keymap)
- 7 new `Command` variants: `TreeUp`, `TreeDown`, `TreeToggle`, `TreeExpand`, `TreeCollapseOrParent`, `TreeHome`, `TreeEnd`
- Children inserted into flat `Vec<TreeNode>` after parent on expand, drained on collapse
- Selection highlight with `tree_selection_bg` theme color

---

### Task 3.3 — File Tree: .gitignore Filtering [DONE]

Respect `.gitignore` rules when displaying the file tree.

**Acceptance criteria:**
- Files/directories matching `.gitignore` patterns are hidden from the tree
- `node_modules/`, `target/`, `.git/` hidden by default (even without `.gitignore`)
- Nested `.gitignore` files are respected
- A toggle command shows/hides ignored files (`Ctrl+Shift+H` or command)

**Implementation details:**
- Use the `ignore` crate (same as ripgrep) for `.gitignore` pattern matching
- Build a `WalkBuilder` to get the list of non-ignored paths
- Add a config option `tree.show_hidden` and `tree.show_ignored`

---

### Task 3.4 — File Tree: Create, Delete, Rename [DONE]

Basic file operations from the tree panel.

**Acceptance criteria:**
- `n` creates a new file: shows an inline text input at the current position, type name, Enter confirms, Esc cancels
- `N` (Shift+N) creates a new directory (same flow)
- `d` deletes the selected file/directory (with confirmation dialog: "Delete foo.rs? [y/N]")
- `r` renames: shows inline text input pre-filled with current name
- After any operation, the tree refreshes to reflect changes
- Operations work on the actual filesystem

**Implementation details:**
- Inline text input: a small single-line text field rendered in-place in the tree
- Add `TreeAction` enum: `None`, `Creating { is_dir: bool, input: String }`, `Renaming { node_idx, input: String }`, `ConfirmDelete { node_idx }`
- Filesystem operations: `std::fs::create_dir_all`, `std::fs::write`, `std::fs::remove_file`, `std::fs::remove_dir_all`, `std::fs::rename`

---

### Task 3.5 — File Tree: File Icons (Nerd Font) ✅

Display file type icons using Nerd Font characters.

**Acceptance criteria:**
- Each file shows an icon based on its extension (e.g., `` for Rust, `` for JS, `` for Python, `` for folders)
- Icons are colored according to the file type
- If Nerd Font is not available, fall back to no icons (toggle: `Ctrl+I`)
- At least 30 file type mappings (common languages + config files)

**Implementation details:**
- Created `axe-tree/src/icons.rs` module with `match`-based lookup (filename first, then extension)
- `show_icons` flag on `FileTree` (default: true), toggled via `Ctrl+I` / `Command::ToggleIcons`
- Directory icons replace arrow prefixes; each icon has its own `Color` rendered as a separate `Span`
- 30+ file type mappings covering languages, web, config, docs, and special files

---

## Phase 4: Editor — Core Text Editing

### Task 4.1 — Editor: Open File from Tree & Display Content ✅

Selecting a file in the tree opens it in the editor panel.

**Acceptance criteria:**
- [x] `Enter` on a file in the tree opens it in the editor panel
- [x] File content is displayed with line numbers in the gutter
- [x] Gutter has a distinct background color (slightly different from editor bg)
- [x] Long lines are not wrapped (horizontal content goes off-screen for now)
- [x] Gutter width adjusts to the number of digits needed (e.g., 3 digits for 100+ lines)
- [x] Status bar shows: filename, line count, file type

**Implementation details:**
- Create `EditorBuffer` in `axe-editor` using `ropey::Rope`
- `BufferManager` holds `Vec<EditorBuffer>` and `active: usize`
- Tree sends `FileSelected(PathBuf)` event → core opens file → editor displays
- Rendering: iterate visible lines, render gutter + content

---

### Task 4.2 — Editor: Cursor Movement ✅

Navigate within the file using keyboard.

**Acceptance criteria:**
- [x] A visible block cursor is rendered at the current position
- [x] `↑`/`↓`/`←`/`→` move the cursor
- [x] `Home` goes to beginning of line, `End` to end of line
- [x] `Ctrl+Home` goes to beginning of file, `Ctrl+End` to end of file
- [x] `PageUp`/`PageDown` scroll by viewport height
- [x] `Ctrl+←`/`Ctrl+→` move by word
- [x] The current line is highlighted with a subtle background color
- [x] Cursor line number in gutter is highlighted
- [x] Viewport scrolls to keep the cursor visible (scroll margin: 5 lines)
- [x] Status bar shows cursor position: `Ln 42, Col 13`

**Implementation details:**
- `CursorState` struct: `row: usize, col: usize, desired_col: usize` (desired_col for up/down on short lines)
- Scroll state: `scroll_row: usize, scroll_col: usize`
- Clamp cursor to valid positions (not past end of line or file)

---

### Task 4.3 — Editor: Text Insertion & Deletion ✅

Basic text editing: type characters, delete, backspace, enter.

**Acceptance criteria:**
- Typing characters inserts them at cursor position
- `Backspace` deletes character before cursor (or joins lines if at column 0)
- `Delete` deletes character at cursor (or joins with next line if at end of line)
- `Enter` splits the line at cursor position, creates new line with auto-indent (match leading whitespace of current line)
- `Tab` inserts spaces (number from config, default 4)
- Content is stored in the rope, all operations use rope API
- File can be saved with `Ctrl+S` (writes rope content to the file path)
- Modified indicator: tab/title shows `●` or `[+]` when buffer has unsaved changes

**Implementation details:**
- All edits go through an `Edit` struct: `{ position, old_text, new_text }`
- Use `ropey::Rope::insert` and `ropey::Rope::remove`
- For `Ctrl+S`: write `rope.write_to(BufWriter::new(File::create(path)?))` — ropey has efficient file writing

---

### Task 4.4 — Editor: Undo/Redo ✅

Undo and redo support for all edits.

**Acceptance criteria:**
- [x] `Ctrl+Z` undoes the last edit
- [x] `Ctrl+Shift+Z` (or `Ctrl+Y`) redoes the last undone edit
- [x] Undo restores the cursor to where the edit was made
- [x] Multiple rapid edits (e.g., typing a word) are grouped into a single undo step
- [x] Undo/redo works across save operations (saving does not clear history)

**Implementation details:**
- `EditHistory` struct with undo stack and redo stack
- Each entry is a `Vec<Edit>` (grouped edits)
- Group edits by time: if the next edit arrives within 500ms and is at a contiguous position, append to current group
- On undo: apply reverse of each edit in the group (in reverse order)
- On redo: apply each edit in the group (in forward order)
- Any new edit after undo clears the redo stack

---

### Task 4.5 — Editor: Selection, Copy, Cut, Paste

Text selection and clipboard operations.

**Acceptance criteria:**
- `Shift+Arrow` starts/extends selection (character-level)
- `Shift+Home`/`Shift+End` selects to beginning/end of line
- `Shift+Ctrl+Home`/`Shift+Ctrl+End` selects to beginning/end of file
- `Ctrl+Shift+←`/`Ctrl+Shift+→` selects by word
- `Ctrl+A` selects all text
- Selected text is highlighted with a distinct background color
- `Ctrl+C` copies selection to system clipboard
- `Ctrl+X` cuts selection (copy + delete)
- `Ctrl+V` pastes from system clipboard at cursor (replaces selection if any)
- `Delete` or `Backspace` with active selection deletes selected text
- Typing with active selection replaces it

**Implementation details:**
- `Selection` struct: `anchor: Position, cursor: Position` (anchor stays, cursor moves)
- Use `arboard` crate for system clipboard access
- Selection range: normalize anchor/cursor to get (start, end) regardless of direction

---

### Task 4.6 — Editor: Search in File (Ctrl+F) ✅

Find text within the current buffer.

**Acceptance criteria:**
- `Ctrl+F` opens a search bar at the top of the editor panel
- Typing in the search bar highlights all matches in the buffer (distinct highlight color)
- `Enter` jumps to next match, `Shift+Enter` jumps to previous match
- Match count is shown: "3 of 17 matches"
- `Esc` closes the search bar and removes highlights
- Case-sensitive toggle: `Alt+C` or a button
- Regex toggle: `Alt+R` or a button
- Search wraps around the file

**Implementation details:**
- Use `regex` crate for regex mode, simple `str::find` for literal mode
- Store match positions as `Vec<Range<usize>>` (byte offsets in rope)
- Current match index tracks which one is "active" (shown with a different highlight)

---

### Task 4.7 — Editor: Multiple Buffers & Tab Bar ✅

Support multiple open files with a tab bar.

**Acceptance criteria:**
- Each opened file gets a tab in the tab bar above the editor
- Tab shows filename (not full path) and modified indicator
- Clicking a tab (mouse) switches to that buffer
- `Ctrl+Tab` switches to the next buffer, `Ctrl+Shift+Tab` to previous
- `Ctrl+W` closes the active buffer (with "save changes?" dialog if modified)
- If the same file is opened again, switch to the existing tab instead of creating a duplicate
- Tab bar scrolls horizontally if there are too many tabs
- Active tab has a distinct style (brighter background, underline or bold)

**Implementation details:**
- `BufferManager` has `buffers: Vec<EditorBuffer>` and `active: usize`
- Tab bar is a widget in `axe-ui` rendered above the editor area
- Opening a file: check if path already exists in buffers, if so just switch active
- Close buffer: remove from vec, adjust active index

---

## Phase 5: Syntax Highlighting

### Task 5.1 — Tree-sitter Integration: Basic Highlighting ✅

Add syntax highlighting using tree-sitter for the most common languages.

**Acceptance criteria:**
- Rust, Go, Python, JavaScript, TypeScript, C, C++, HTML, CSS, JSON, TOML, Markdown, Bash files are syntax highlighted
- Keywords, strings, comments, functions, types, variables have distinct colors
- Highlighting updates incrementally as the user types (not full re-parse)
- Highlighting uses the theme's syntax color map
- Files without a known language are displayed without highlighting (plain text)

**Implementation details:**
- Add `tree-sitter` and relevant `tree-sitter-{lang}` crates to `axe-editor`
- Language detection by file extension
- Use highlight queries (`.scm` files) — bundle the Helix project's query files (they're well maintained)
- On each edit: call `tree.edit()` with the edit range, then re-parse incrementally
- Map highlight capture names to theme colors: `keyword` → theme.syntax["keyword"], etc.
- Store `Parser` and `Tree` in `EditorBuffer`

---

### Task 5.2 — Theme Engine: Load Themes from TOML ✅

Load syntax and UI colors from TOML theme files.

**Acceptance criteria:**
- [x] Ship at least two built-in themes: `axe-dark` (default) and `axe-light`
- [x] Theme file defines all colors: UI chrome, gutter, diagnostics, syntax scopes
- [x] Syntax scopes map to tree-sitter capture names
- [x] Config option: `ui.theme = "axe-dark"` selects the theme
- [x] Theme files are loaded from `~/.config/axe/themes/` (user themes) or bundled
- [ ] Changing the theme via command palette reloads all colors immediately (requires Task 8.2 — Command Palette)

**Implementation details:**
- Theme TOML structure as defined in `AXE_ARCHITECTURE.md` section 3.6
- Parse with `serde` + `toml`
- Store in `axe-config`, pass to `axe-ui` for rendering
- Syntax style: `{ fg, bg, bold, italic, underline }` per capture name

---

## Phase 6: Integrated Terminal

### Task 6.1 — Terminal: Spawn Shell & Display Output ✅ DONE

Spawn a shell process in the terminal panel and display its output.

**Acceptance criteria:**
- Terminal panel runs the user's `$SHELL` (or `/bin/bash` fallback)
- Shell output is displayed in the terminal panel in real-time
- Prompt is visible and responsive
- Colors from the shell (ANSI escape codes) are rendered correctly
- Terminal fills the available panel area and resizes with the panel

**Implementation details:**
- Use `portable-pty` to create a PTY and spawn the shell
- Use `alacritty_terminal` for VT parsing and terminal state management
- Background `std::thread` reads PTY output and sends it via `mpsc` channel to the main loop
- Render the `alacritty_terminal::Term` grid as Ratatui cells with ANSI color conversion

---

### Task 6.2 — Terminal: Keyboard Input ✅

Pass keyboard input to the terminal shell.

**Acceptance criteria:**
- When terminal panel is focused, all keypresses are forwarded to the PTY
- Typing commands works: `ls`, `cd`, `cargo build`, etc.
- Special keys work: arrows (command history), Tab (completion), Ctrl+C (interrupt)
- Ctrl+D sends EOF
- Interactive programs work (vim, top, htop in the terminal panel)
- Escape sequences for special keys are correctly translated

**Implementation details:**
- When focus is Terminal, convert `crossterm::KeyEvent` to bytes and write to PTY
- Use `alacritty_terminal`'s key encoding or manually encode special keys
- Pass-through mode: only intercept explicitly bound keys (like `Ctrl+Shift+T` for new tab), forward everything else

---

### Task 6.3 — Terminal: Multiple Tabs ✅

Support multiple terminal instances with tabs.

**Acceptance criteria:**
- `Ctrl+Shift+T` creates a new terminal tab (new PTY + shell)
- Tab bar at the top of terminal panel shows all tabs
- `Ctrl+Shift+W` closes the current terminal tab
- `Ctrl+Shift+←`/`Ctrl+Shift+→` switches between terminal tabs (or mouse click)
- Tab shows the running process name (or shell name)
- When the last tab is closed, terminal panel stays visible with a "No terminals" message
- Maximum 10 terminal tabs

**Implementation details:**
- `TerminalManager` holds `Vec<TerminalTab>` and `active: usize`
- Each tab is independent: own PTY, own shell process, own scrollback
- When closing a tab, send SIGHUP to the child process

---

### Task 6.4 — Terminal: Scrollback Buffer ✅

Scroll through terminal output history.

**Acceptance criteria:**
- Terminal has a scrollback buffer (default: 10,000 lines)
- `Shift+PageUp` / `Shift+PageDown` scrolls through history
- `Shift+Home` scrolls to the top of history
- `Shift+End` scrolls to the bottom (current output)
- Scrollbar indicator on the right side when not at the bottom
- New output auto-scrolls to bottom (unless user has scrolled up)

**Implementation details:**
- `alacritty_terminal` handles scrollback buffer internally
- Expose scroll position to the renderer
- Track `is_user_scrolled` flag: if true, don't auto-scroll

---

## Phase 7: LSP Integration

### Task 7.1 — LSP: Client Infrastructure [DONE]

Set up the LSP client that can start, communicate with, and stop language servers.

**Acceptance criteria:**
- [x] `LspManager` can start a language server process given a command + args
- [x] Communication via JSON-RPC over stdin/stdout
- [x] `initialize` handshake completes successfully
- [x] `textDocument/didOpen` sent when a file is opened
- [x] `textDocument/didChange` sent on each edit (incremental sync)
- [x] `textDocument/didSave` sent on file save
- [x] Server stdout is parsed correctly (Content-Length header + JSON body)
- [x] If the server crashes, a notification is shown and LSP features gracefully degrade
- [x] LSP servers are configured in the config file per language

**Implementation details:**
- Use `lsp-types` crate for all LSP type definitions
- Spawn server process with `tokio::process::Command`
- JSON-RPC transport: read/write with proper Content-Length framing
- Pending requests tracked with ID → oneshot channel for response
- Server notifications (like diagnostics) sent as events to the main loop

---

### Task 7.2 — LSP: Diagnostics (Errors & Warnings) [DONE]

Display LSP diagnostics in the editor.

**Acceptance criteria:**
- [x] Errors shown as red indicators in the gutter (e.g., `✖` or `●`)
- [x] Warnings shown as yellow indicators (e.g., `▲` or `●`)
- [x] Hints and info shown as blue indicators
- [x] Error/warning text shown in the status bar when cursor is on a diagnostic line
- [x] Diagnostic underlines in the editor text (wavy underline if terminal supports it, or colored underline)
- [x] Total error/warning count in status bar: `E:N W:M`
- [x] Diagnostics update after each save or edit (depending on server capability)

**Implementation details:**
- `publishDiagnostics` notification → store `Vec<Diagnostic>` on the buffer
- Map LSP positions (line, character) to rope positions
- Render gutter icons and text decorations during the editor render pass

---

### Task 7.3 — LSP: Autocomplete [DONE]

Code completion popup triggered by typing or manual invocation.

**Acceptance criteria:**
- [x] Completion popup appears automatically after typing `.` or `:` trigger characters
- [x] `Alt+/` (or `F3` fallback) manually triggers completion
- [x] Popup shows completion items with label, kind icon (function, variable, type, etc.), and optional detail
- [x] `↑`/`↓` navigates the list, `Enter` or `Tab` accepts the selected item
- [x] `Esc` dismisses the popup
- [x] Accepted completion replaces the current word/prefix correctly
- [x] Popup positioned at the cursor, does not go off-screen (flips direction if needed)

**Implementation details:**
- `axe-core/src/completion.rs`: `CompletionState`, `CompletionItem`, `CompletionKind`, response parsing, filtering
- `axe-lsp/src/client.rs`: `PendingRequestKind` enum, `send_request()` / `take_pending()` for response routing, completion capability in `initialize_params`
- `axe-lsp/src/manager.rs`: `request_completion()`, `CompletionResponse` event routing via `poll_events()`
- Non-modal key handling: typing continues while popup is open, filter updates dynamically
- `apply_text_edit()` on `EditorBuffer` for range replacement with undo support
- Popup rendered at cursor position with kind icons, selection highlight, scroll, auto-flip above if insufficient space below

---

### Task 7.4 — LSP: Go To Definition & References [DONE]

Navigate to symbol definitions and find all references.

**Acceptance criteria:**
- [x] `F12` (or `gd` in modal mode) goes to the definition of the symbol under cursor
- [x] If definition is in another file, that file is opened in a new buffer and cursor jumps to the position
- [x] If definition is in the same file, cursor just jumps
- [x] If there are multiple definitions, show a selection overlay
- [x] `Shift+F12` shows all references in an overlay list
- [x] Each reference shows: file path, line number, and the line content
- [x] `Enter` on a reference opens/jumps to that location

**Implementation details:**
- `textDocument/definition` request → `Location` or `LocationLink` response
- `textDocument/references` request → `Vec<Location>` response
- `axe-core/src/location_list.rs`: `LocationItem`, `LocationList`, LSP response parsing (Location, LocationLink, arrays, null)
- `axe-lsp`: `PendingRequestKind::Definition/References`, `LspEvent::DefinitionResponse/ReferencesResponse`, request methods + response routing
- `axe-core/src/app.rs`: overlay key handling (Esc/Enter/Up/Down), single-result direct jump, multi-result overlay, preview buffer auto-promotion with LSP didOpen
- `axe-ui`: `render_location_list()` overlay (60% width, 50% height, selection highlight, scroll)

---

### Task 7.5 — LSP: Hover Information [DONE]

Show type/documentation info when hovering over symbols.

**Acceptance criteria:**
- [x] `K` (in modal mode) or `Ctrl+Shift+K` shows hover info for the symbol under cursor
- [x] Hover tooltip displayed as a floating overlay near the cursor
- [x] Content supports Markdown rendering (basic: bold, italic, code blocks, headers)
- [x] `Esc` or any key dismisses the tooltip
- [x] Mouse hover also triggers (with a short delay, e.g., 500ms)

**Implementation details:**
- `axe-core/src/hover.rs`: `HoverInfo`, `HoverLine`, `HoverSpan` types; `parse_hover_response()` handling all LSP formats (MarkupContent, MarkedString, arrays); `markdown_to_hover_lines()` parser
- `axe-lsp`: `PendingRequestKind::Hover`, `LspEvent::HoverResponse`, `request_hover()`, hover capability in `initialize_params`, response routing in `poll_events()`
- `axe-core/src/app.rs`: `hover_info` field, key dismissal (any key clears, Esc only dismisses hover), `request_hover()` helper, mouse hover with 500ms delay via `check_hover_timer()` / `handle_mouse_moved()`
- `axe-ui`: `render_hover_tooltip()` positioned at cursor (above/below), styled spans (bold/italic/code), overlay theme colors
- Keybindings: `Ctrl+Shift+K` (both case variants) + `F4` fallback, `Command::ShowHover`

---

### Task 7.6 — LSP: Format on Save ✅

Auto-format the document on save using the LSP server.

**Acceptance criteria:**
- When `format_on_save = true` in config, `Ctrl+S` formats before saving
- Formatting uses `textDocument/formatting` LSP request
- Edits from the formatter are applied to the buffer
- If the server doesn't support formatting, save proceeds without formatting
- A manual format command also exists: `Ctrl+Shift+I` or command palette

**Implementation details:**
- Send `textDocument/formatting` request with current tab/indent settings
- Apply returned `TextEdit[]` to the rope (in reverse order to preserve positions)
- Then save the file

---

## Phase 8: Project Search & Overlays

### Task 8.1 — Fuzzy File Finder (Ctrl+P) [DONE]

Quick file search across the project.

**Acceptance criteria:**
- [x] `Ctrl+P` opens a centered overlay with a text input
- [x] Typing filters all project files by fuzzy matching
- [x] Results update in real-time as you type
- [x] Results show relative file path, with matched characters highlighted
- [x] `Up`/`Down` navigates results, `Enter` opens the selected file
- [x] `Esc` closes the finder
- [x] Results are ranked by match quality (best match first)
- [x] Maximum ~1000 files indexed (performance target: < 5ms per keystroke)

**Implementation details:**
- Use `nucleo-matcher` crate for fuzzy matching (from Helix project)
- Build file list using `ignore` crate (respects .gitignore)
- File list built fresh each time Ctrl+P is pressed
- Overlay: centered, 60% width, 50% height

**Implementation notes:**
- `axe-core/src/file_finder.rs`: `FileFinder` struct with `nucleo_matcher::pattern::Pattern` for synchronous matching
- `Command::OpenFileFinder` variant + `Ctrl+P` default keybinding
- Key interception in `handle_key_event()` after confirm_dialog, before resize mode
- `CloseOverlay` closes file finder first, then help overlay
- Rendering in `axe-ui`: centered overlay with input field, highlighted match chars, selection indicator, file count footer

---

### Task 8.2 — Command Palette (Ctrl+Shift+P) [DONE]

Search and execute any available command.

**Acceptance criteria:**
- [x] `Ctrl+Shift+P` opens the command palette overlay
- [x] Lists all available commands with their keybindings shown on the right
- [x] Fuzzy search filters commands as you type
- [x] `Enter` executes the selected command
- [x] `Esc` closes the palette
- [x] Commands are listed with human-readable names (e.g., "File: Save", "View: Toggle Terminal")

**Implementation details:**
- Reuse the same fuzzy finder widget as Ctrl+P, but with commands as the data source
- Each command entry: display name, category, keybinding
- Build command list dynamically from the `Command` enum + keymap

---

### Task 8.3 — Project-Wide Search (Ctrl+Shift+F) [DONE]

Search text across all project files.

**Acceptance criteria:**
- [x] `Ctrl+Shift+F` opens a search overlay
- [x] Text input for search query
- [x] Results grouped by file, showing matching lines with context
- [x] Matched text highlighted in each result line
- [x] `Enter` on a result opens the file and jumps to the line
- [x] Case sensitivity toggle
- [x] Regex toggle
- [x] File pattern include/exclude (e.g., `*.rs`, `!*.test.*`)
- [x] Result count shown: "42 results in 7 files"
- [x] Search runs in background, results stream in progressively

**Implementation details:**
- Uses `ignore` crate (respects .gitignore) + `regex` crate for matching
- Search runs in `std::thread` with `mpsc` channel for results
- Results drained each frame via `try_recv()` for progressive streaming
- F2 as universal fallback keybinding (like F1 for command palette)

---

## Phase 9: Git Integration

### Task 9.1 — Git: Status Bar Branch Name ✅

Show the current git branch in the status bar.

**Acceptance criteria:**
- [x] Status bar shows current branch name with a git icon: `⎇ main`
- [x] Updates when branch changes (checked periodically or on file save)
- [x] If not in a git repo, no git info shown
- [x] Detached HEAD shows the short commit hash

**Implementation details:**
- Use `git2` crate (or `gix`)
- `Repository::open(project_root)` → `repo.head()` → branch name
- Check on startup and after each file save

**Implementation notes:**
- `axe-core/src/git.rs`: `current_branch()` using `git2::Repository::discover()`, returns branch name or 7-char short hash for detached HEAD
- `AppState`: `git_branch: Option<String>` + `last_git_branch_check` fields, `refresh_git_branch()` (5s interval) + `force_refresh_git_branch()` after save
- `axe-ui`: rendered between diagnostic counts and status message in `build_status_bar()`

---

### Task 9.2 — Git: Gutter Diff Indicators

Show which lines have been added, modified, or deleted compared to the last commit.

**Acceptance criteria:**
- Added lines: green `+` or `▎` bar in gutter
- Modified lines: blue `~` or `▎` bar in gutter
- Deleted lines: red `_` or `▁` triangle at the deletion point
- Indicators update after saving
- Only shown for files tracked by git

**Implementation details:**
- `git2::Repository::diff_index_to_workdir` for unstaged changes
- Map diff hunks to line ranges in the current buffer
- Store `diff_hunks: Vec<DiffHunk>` on `EditorBuffer`
- Render in gutter alongside line numbers and diagnostic indicators

---

### Task 9.3 — Git: File Tree Status Icons

Show git status for files in the file tree.

**Acceptance criteria:**
- Modified files: `M` badge or colored filename (e.g., yellow)
- New/untracked files: `U` badge or colored (e.g., green)
- Deleted files: shown with strikethrough or red color
- Ignored files: dimmed (if shown)
- Directories containing modified files: show a dot indicator
- Status updates after file save or git operations

**Implementation details:**
- `git2::Repository::statuses` gives status for all files
- Map status to `GitStatus` enum on each `TreeNode`
- Color/badge applied during tree rendering

---

## Phase 10: Configuration & Polish

### Task 10.1 — Configuration File Loading ✅

Load user configuration from TOML file.

**Acceptance criteria:**
- [x] On startup, load `~/.config/axe/config.toml` if it exists
- [x] Project-level `.axe/config.toml` overrides global config
- [x] All keybindings configurable (`[keybindings]` section with key combo parsing)
- [x] All theme colors configurable (theme TOML files with `from_theme_file`)
- [x] Editor settings: tab size, spaces vs tabs, word wrap, format on save
- [x] Tree settings: show hidden, show icons, sort order
- [x] Terminal settings: shell command, scrollback size
- [x] If config file has errors, show a notification and use defaults
- [ ] `ReloadConfig` command re-reads config without restarting (deferred — needs runtime re-apply of all settings)

**Implementation details:**
- Deserialize with `serde` + `toml`
- Merge strategy: project config fields override global, missing fields fall through to defaults
- All config fields have sane defaults (app works without any config file)

---

### Task 10.2 — Session Save & Restore

Remember open files and layout between sessions.

**Acceptance criteria:**
- On quit, save session to `.axe/session.json`: open buffers (paths + cursor positions + scroll), layout (panel sizes, visibility), active buffer, expanded tree nodes
- On startup in the same project, restore session: re-open files, restore cursor positions, restore layout
- `--no-session` flag skips session restore
- If a previously open file no longer exists, skip it and show a notification

**Implementation details:**
- `Session` struct serialized with `serde_json`
- Save on clean quit, skip on force quit
- Load before entering the event loop

---

### Task 10.3 — Status Bar Polish

Complete and polish the status bar.

**Acceptance criteria:**
- Left side: mode indicator (INSERT/NORMAL/RESIZE), filename, modified indicator
- Center: notifications (temporary messages that fade after 3 seconds)
- Right side: file type, encoding (UTF-8), line ending (LF/CRLF), cursor position `Ln X, Col Y`, git branch, diagnostic counts
- Status bar has a distinct background color from the theme
- Each section is styled differently (e.g., mode indicator has colored background)
- Status bar updates in real-time

---

### Task 10.4 — Startup Screen

Show a welcome screen when Axe is opened without a file or project.

**Acceptance criteria:**
- ASCII art logo "AXE" displayed centered
- Version number below the logo
- List of key shortcuts: "Ctrl+P: Open file", "Ctrl+N: New file", etc.
- Recent projects/files list (if session history exists)
- Disappears as soon as a file is opened or any editing begins

---

## Phase 11: Advanced Features (Future)

### Task 11.1 — Find and Replace
### Task 11.2 — Go to Line (Ctrl+G)
### Task 11.3 — Auto-close brackets and quotes
### Task 11.4 — Bracket matching and highlighting
### Task 11.5 — Indent guides
### Task 11.6 — Code folding (tree-sitter based)
### Task 11.7 — LSP signature help
### Task 11.8 — LSP code actions
### Task 11.9 — LSP rename symbol
### Task 11.10 — LSP inlay hints
### Task 11.11 — Multiple cursors
### Task 11.12 — Plugin system (Lua)
### Task 11.13 — Debugger integration (DAP)
### Task 11.14 — Git blame / diff viewer
### Task 11.15 — Remote development (SSH)

---

## Notes for Claude Code

1. **Always reference `AXE_ARCHITECTURE.md`** for struct definitions, enum variants, and design decisions.
2. **Each task must compile and run** after completion. Never leave the project in a broken state.
3. **Write tests** for non-UI logic (rope operations, keymap resolution, config parsing, tree operations).
4. **Use English in code** (comments, variable names, log messages).
5. **Commit message format:** `feat(crate): short description` — e.g., `feat(axe-editor): add cursor movement`
6. **When in doubt**, favor simplicity. Implement the minimal version first, refine later.
7. **Performance matters** from day one for the editor: use rope operations correctly, don't clone strings unnecessarily, render only visible lines.


