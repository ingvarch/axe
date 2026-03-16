# Axe — Terminal IDE Architecture Document

**Project Name:** Axe
**Language:** Rust
**UI Framework:** Ratatui + Crossterm
**Status:** Design Phase
**Last Updated:** March 2026

---

## 1. Vision & Philosophy

Axe is a terminal-based IDE built from scratch in Rust. Unlike tmux/zellij (generic terminal multiplexers) or plugin-heavy Neovim setups, Axe is a **purpose-built IDE** that understands projects, integrates all components natively, and provides a cohesive editing experience — similar to how k9s is a purpose-built tool for Kubernetes rather than a collection of shell scripts.

### Core Principles

1. **Project-aware, not file-aware.** Axe understands project structure, build systems, VCS — not just individual files.
2. **Native integration over composition.** Components communicate through a shared event bus, not pipes between separate processes.
3. **Fast by default.** Diff-based rendering, rope data structures, async I/O. No perceptible lag on any operation.
4. **Beautiful TUI.** TrueColor, gradients, animations, polished default theme. Terminal apps don't have to look like 1985.
5. **Extensible.** Plugin system, configurable keybindings, scriptable via Lua or WASM (future).

---

## 2. High-Level Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      axe (binary)                      │
│  ┌────────────────────────────────────────────────────┐  │
│  │                    axe-ui                        │  │
│  │  Layout Manager · Theme Engine · Overlay Stack     │  │
│  │  Status Bar · Tab Bar · Command Palette            │  │
│  └────────────────────┬───────────────────────────────┘  │
│                       │ renders                          │
│  ┌────────────────────▼───────────────────────────────┐  │
│  │                  axe-core                        │  │
│  │  AppState · Event Bus · Command Dispatcher         │  │
│  │  Focus Manager · Keymap Resolver · Plugin Host     │  │
│  └──┬──────────┬──────────┬──────────┬──────────┬─────┘  │
│     │          │          │          │          │         │
│  ┌──▼──┐  ┌───▼───┐  ┌──▼───┐  ┌──▼──┐  ┌───▼──────┐  │
│  │tree │  │editor │  │term  │  │ lsp │  │  config  │  │
│  └─────┘  └───────┘  └──────┘  └─────┘  └──────────┘  │
└──────────────────────────────────────────────────────────┘
```

### Dependency Graph

```
axe-config ──────────────────────────────┐
                                           ▼
axe-tree ────► axe-core ◄──── axe-editor
                    ▲   ▲
axe-terminal ─────┘   └────── axe-lsp
                    │
                    ▼
                axe-ui (renders everything)
```

**Rule:** All crates depend on `axe-core`. No crate depends on another crate directly (except through core). This ensures components are decoupled and independently testable.

---

## 3. Crate Breakdown

### 3.1 `axe-core` — Central Hub

The brain of the application. Owns the global state and routes all communication.

#### AppState

```rust
pub struct AppState {
    /// Which panel currently has keyboard focus
    pub focus: FocusTarget,

    /// Active project root path
    pub project: ProjectState,

    /// Stack of overlay windows (file finder, command palette, dialogs)
    pub overlays: Vec<Box<dyn Overlay>>,

    /// All open editor buffers
    pub buffers: BufferManager,

    /// Configuration (keybindings, theme, LSP servers, etc.)
    pub config: AppConfig,

    /// Whether the app should quit
    pub should_quit: bool,

    /// Notification queue (status messages, errors)
    pub notifications: VecDeque<Notification>,
}

pub enum FocusTarget {
    Tree,
    Editor,
    Terminal(usize),  // terminal tab index
    Overlay,          // topmost overlay gets focus
}
```

#### Event System

All inter-component communication happens through events. Components never call each other directly.

```rust
pub enum Event {
    // === Input Events ===
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),

    // === Command Events (from keybindings or command palette) ===
    Command(Command),

    // === Editor Events ===
    FileOpened { path: PathBuf, content: String },
    FileSaved { path: PathBuf },
    FileModified { path: PathBuf },
    BufferChanged { path: PathBuf, changes: Vec<TextChange> },
    CursorMoved { path: PathBuf, position: Position },

    // === File Tree Events ===
    FileSelected(PathBuf),
    DirectoryToggled(PathBuf),
    FileCreated(PathBuf),
    FileDeleted(PathBuf),
    FileRenamed { from: PathBuf, to: PathBuf },

    // === Terminal Events ===
    TerminalOutput { tab: usize, data: Vec<u8> },
    TerminalExited { tab: usize, code: i32 },

    // === LSP Events ===
    LspStarted { language: String },
    LspStopped { language: String },
    Diagnostics { path: PathBuf, diagnostics: Vec<Diagnostic> },
    Completions { items: Vec<CompletionItem> },
    Definition { locations: Vec<Location> },
    Hover { content: HoverContent },
    References { locations: Vec<Location> },
    CodeActions { actions: Vec<CodeAction> },

    // === Git Events ===
    GitStatusChanged(Vec<GitFileStatus>),
    GitBranchChanged(String),

    // === UI Events ===
    Tick,  // periodic refresh (e.g., 60fps timer)

    // === Plugin Events ===
    PluginEvent { plugin_id: String, data: serde_json::Value },
}
```

#### Command System

Every IDE action is a named command. Keybindings map to commands. The command palette lists all available commands. Plugins register new commands.

```rust
pub enum Command {
    // --- File Operations ---
    NewFile,
    OpenFile(PathBuf),
    Save,
    SaveAs,
    SaveAll,
    CloseBuffer,
    CloseAllBuffers,

    // --- Navigation ---
    OpenFileFinder,
    OpenProjectSearch,
    OpenCommandPalette,
    GoToLine,
    GoToDefinition,
    GoToReferences,
    GoToImplementation,
    GoToNextDiagnostic,
    GoToPreviousDiagnostic,
    SwitchBuffer,
    JumpBack,
    JumpForward,

    // --- Editing ---
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    FindAndReplace,
    ToggleComment,
    FormatDocument,
    FormatSelection,
    RenameSymbol,
    CodeAction,

    // --- View / Layout ---
    FocusTree,
    FocusEditor,
    FocusTerminal,
    ToggleTree,
    ToggleTerminal,
    ToggleStatusBar,
    SplitVertical,
    SplitHorizontal,
    CloseSplit,
    ZoomPanel,          // maximize active panel (toggle)
    ResetLayout,        // restore default panel sizes
    EnterResizeMode,    // enter panel resize mode (zellij-style)
    ExitResizeMode,     // exit resize mode (also Esc)
    ResizeGrow,         // grow active panel in focused direction
    ResizeShrink,       // shrink active panel in focused direction
    ResizeLeft,         // move left border left (grow) or right border left (shrink)
    ResizeRight,        // move right border right
    ResizeUp,           // move top border up
    ResizeDown,         // move bottom border down
    EqualizeLayout,     // make all panels equal size
    SwapPanelNext,      // swap active panel with next
    SwapPanelPrevious,  // swap active panel with previous
    IncreaseFontSize,
    DecreaseFontSize,

    // --- Terminal ---
    NewTerminalTab,
    CloseTerminalTab,
    NextTerminalTab,
    PreviousTerminalTab,
    ClearTerminal,
    RunBuildCommand,
    RunTestCommand,
    RunCustomCommand(String),

    // --- Git ---
    GitStatus,
    GitDiff,
    GitCommit,
    GitPush,
    GitPull,
    GitBlame,
    GitLog,

    // --- Misc ---
    OpenSettings,
    ReloadConfig,
    ToggleFullscreen,
    Quit,
    ForceQuit,

    // --- Plugin-Defined ---
    Custom { id: String, args: Option<serde_json::Value> },
}
```

#### Keymap Resolver

Keybindings are context-sensitive. The same key can do different things depending on the active panel and mode.

```rust
pub struct KeymapResolver {
    /// Global keybindings (active everywhere)
    global: HashMap<KeyCombo, Command>,

    /// Context-specific keybindings (override global)
    contexts: HashMap<KeymapContext, HashMap<KeyCombo, Command>>,
}

pub enum KeymapContext {
    Editor,
    EditorInsertMode,
    EditorNormalMode,
    EditorVisualMode,
    Tree,
    Terminal,
    FileFinder,
    CommandPalette,
    Dialog,
    ResizeMode,  // panel resize mode (zellij-style)
}
```

Resolution order: Overlay context → Active panel context → Global. First match wins.

#### Panel Resize Mode (zellij-style)

Axe supports a dedicated **resize mode**, inspired by zellij. When entering resize mode, the panel borders become highlighted and arrow keys resize the active panel instead of their normal function.

**How it works:**

1. User presses `Ctrl+Alt+R` → enters resize mode
2. Status bar shows `-- RESIZE --` indicator (similar to Vim's mode indicator)
3. Panel borders change color (e.g., bright yellow) to indicate resize mode is active
4. Arrow keys resize the active panel:
   - `←` / `→` — adjust width of the active panel (tree or editor splits)
   - `↑` / `↓` — adjust height of the active panel (editor vs terminal split)
   - `+` / `=` — grow active panel by a larger step (5%)
   - `-` — shrink active panel by a larger step (5%)
   - `0` — equalize all panels to equal sizes
5. `Esc` or `Enter` or `q` → exits resize mode, borders return to normal

**Resize behavior per panel:**

| Active Panel | `←` | `→` | `↑` | `↓` |
|-------------|-----|-----|-----|-----|
| File Tree | shrink width | grow width | — | — |
| Editor | grow left / shrink right (depends on split) | grow right / shrink left | grow (take from terminal) | shrink (give to terminal) |
| Terminal | — | — | grow (take from editor) | shrink (give to editor) |

**Implementation:**

```rust
pub struct ResizeModeState {
    /// Whether resize mode is active
    pub active: bool,

    /// Step size for arrow key resize (in percentage points)
    pub step_small: u16,  // default: 2

    /// Step size for +/- resize
    pub step_large: u16,  // default: 5

    /// Minimum panel size (percentage) — prevents collapsing to zero
    pub min_size_pct: u16,  // default: 10
}
```

**Mouse resize (always available, no mode required):**

Users can also resize panels by clicking and dragging panel borders at any time — this does not require entering resize mode. The cursor changes to a resize indicator when hovering over a border (in terminals that support cursor shapes).

**Zoom toggle:**

`Ctrl+Alt+Z` (or `ZoomPanel` command) maximizes the active panel to fill the entire screen, hiding all other panels. Press again to restore the previous layout. This is useful for focusing on the editor or terminal temporarily.

---

### 3.2 `axe-editor` — Code Editor

The most complex module. A full code editor widget built from scratch.

#### Data Structures

```rust
pub struct EditorBuffer {
    /// Rope-based text storage (ropey crate)
    pub content: Rope,

    /// File path (None for unscratchpad buffers)
    pub path: Option<PathBuf>,

    /// Cursor state
    pub cursor: CursorState,

    /// Selection state (None if no selection)
    pub selection: Option<Selection>,

    /// Undo/redo history
    pub history: EditHistory,

    /// Syntax tree (tree-sitter)
    pub syntax_tree: Option<Tree>,

    /// Language configuration
    pub language: Option<LanguageConfig>,

    /// Dirty flag (unsaved changes)
    pub modified: bool,

    /// Scroll offset (viewport)
    pub scroll: ScrollState,

    /// Diagnostics from LSP
    pub diagnostics: Vec<Diagnostic>,

    /// Git diff hunks
    pub diff_hunks: Vec<DiffHunk>,

    /// Breakpoints (for future debugger integration)
    pub breakpoints: HashSet<usize>,

    /// Marks / bookmarks
    pub marks: HashMap<char, Position>,

    /// Soft wrap or horizontal scroll
    pub wrap_mode: WrapMode,
}
```

#### Text Storage: Rope (via `ropey`)

Why rope and not `String` or `Vec<String>`:

| Operation | String/Vec | Rope |
|-----------|-----------|------|
| Insert at line N | O(n) — must shift everything after | O(log n) |
| Delete range | O(n) | O(log n) |
| Line indexing | O(n) or pre-computed | O(log n) |
| Memory for 1M lines | Contiguous (bad for fragmentation) | Chunks (cache-friendly) |

Ropey is the same library Helix uses. Battle-tested for code editors.

#### Syntax Highlighting: Tree-sitter

Tree-sitter provides incremental parsing — when the user types, only the changed portion of the AST is re-parsed, not the entire file.

```rust
pub struct SyntaxHighlighter {
    /// Tree-sitter parser instance
    parser: Parser,

    /// Current syntax tree
    tree: Option<Tree>,

    /// Highlight query for the current language
    highlight_query: Query,

    /// Language grammar
    language: Language,
}
```

Tree-sitter grammars are loaded as shared libraries (`.so`/`.dylib`) or compiled into the binary. Each language needs:
- A grammar (parser)
- A highlight query (`highlights.scm`)
- Optionally: injections query (for embedded languages), indents query, textobjects query

The Helix project maintains excellent query files for 100+ languages — these can be reused.

#### Edit Modes

Axe should support multiple editing paradigms (configurable):

```rust
pub enum EditMode {
    /// Modal editing (Normal → Insert → Visual), inspired by Vim/Helix
    Modal(ModalState),

    /// Standard editing (always insert, Ctrl+C/V/Z shortcuts)
    Standard,
}

pub enum ModalState {
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
    Command,  // ':' command line mode
}
```

Default should be Standard mode (lower barrier to entry), with Modal mode as an option in config.

#### Gutter

The gutter (left side of editor) shows:

```
 1 │ ● │ fn main() {
 2 │   │     let x = 42;
 3 │ ! │     println!("{}", x);  // ← LSP warning
 4 │ + │     let y = x + 1;     // ← git: new line
 5 │   │ }
```

Columns: line numbers, diagnostics (errors/warnings), git diff indicators (+/-/~), breakpoints (future).

#### Undo/Redo

Using a tree-based undo system (not linear). This means branching edits are preserved:

```rust
pub struct EditHistory {
    /// All recorded edit states
    nodes: Vec<HistoryNode>,

    /// Current position in the history tree
    current: usize,
}

pub struct HistoryNode {
    /// The edit that was applied
    edit: TextChange,

    /// Parent node index
    parent: Option<usize>,

    /// Child node indices (branches)
    children: Vec<usize>,

    /// Timestamp
    timestamp: Instant,
}
```

#### Editor Features Roadmap

| Feature | Priority | Complexity | Notes |
|---------|----------|------------|-------|
| Basic text editing (insert/delete/newline) | P0 — MVP | Medium | Rope + cursor management |
| Line numbers | P0 — MVP | Low | Gutter rendering |
| Syntax highlighting (tree-sitter) | P0 — MVP | Medium | Incremental parsing |
| Search in file (Ctrl+F) | P0 — MVP | Low | Regex via `regex` crate |
| Search and replace | P1 | Low | Extension of search |
| Undo/Redo (tree-based) | P0 — MVP | Medium | Edit history tree |
| Multiple buffers / tabs | P0 — MVP | Low | BufferManager |
| Selection (single) | P0 — MVP | Medium | Anchor + cursor |
| Copy/Cut/Paste (system clipboard) | P0 — MVP | Low | `arboard` or `cli-clipboard` crate |
| Soft word wrap | P1 | Medium | Virtual lines |
| Multiple selections/cursors | P2 | High | Helix-style |
| Code folding | P2 | Medium | Tree-sitter fold queries |
| Minimap | P3 | Medium | Condensed rendering |
| Indent guides | P1 | Low | Virtual decoration |
| Bracket matching/highlighting | P1 | Low | Tree-sitter |
| Auto-indent on Enter | P1 | Medium | Tree-sitter indent queries |
| Auto-close brackets/quotes | P1 | Low | Character pair rules |
| Snippet expansion | P2 | High | LSP snippets + custom |
| Rectangular/column selection | P2 | Medium | Block cursor mode |
| Macro recording/playback | P3 | Medium | Command sequence recording |

---

### 3.3 `axe-tree` — File Tree Panel

Project-aware file browser on the left side of the IDE.

#### Structure

```rust
pub struct FileTree {
    /// Root directory of the project
    root: PathBuf,

    /// Tree nodes (flat vec with parent indices for efficiency)
    nodes: Vec<TreeNode>,

    /// Currently selected node index
    selected: usize,

    /// Scroll offset
    scroll: usize,

    /// Filter patterns (gitignore, custom)
    filters: Vec<GlobPattern>,

    /// Watcher for filesystem changes
    watcher_rx: Receiver<FileSystemEvent>,
}

pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: NodeKind,
    pub depth: usize,
    pub expanded: bool,       // for directories
    pub children_loaded: bool, // lazy loading
    pub git_status: Option<GitStatus>,
    pub parent: Option<usize>,
}

pub enum NodeKind {
    File { size: u64, language: Option<String> },
    Directory { child_count: usize },
    Symlink { target: PathBuf },
}
```

#### Features

| Feature | Priority | Notes |
|---------|----------|-------|
| Recursive tree display with expand/collapse | P0 — MVP | Arrow keys or Enter to toggle |
| Lazy loading (load children on expand) | P0 — MVP | Don't read entire tree at startup |
| .gitignore filtering | P0 — MVP | `ignore` crate (same as ripgrep) |
| File icons (Nerd Font) | P1 | Map extension → icon character |
| Git status indicators (M/A/D/?) | P1 | `git2` crate |
| Create / delete / rename files | P1 | Inline text input for name |
| Fuzzy file finder (Ctrl+P) | P0 — MVP | `nucleo` crate (from Helix) |
| File preview on hover/select | P2 | Show first N lines in floating window |
| Drag and drop (mouse) | P3 | Reorder / move files |
| Custom sorting (name, modified, type) | P2 | Configurable |
| Filesystem watcher (auto-refresh) | P1 | `notify` crate |
| Search within tree (filter-as-you-type) | P1 | Dim non-matching nodes |
| Workspace / multi-root support | P3 | Multiple project roots |

---

### 3.4 `axe-terminal` — Embedded Terminal

A full terminal emulator embedded as a panel.

#### Architecture

```rust
pub struct TerminalManager {
    /// Terminal tabs
    pub tabs: Vec<TerminalTab>,

    /// Active tab index
    pub active: usize,
}

pub struct TerminalTab {
    /// Tab title (customizable)
    pub title: String,

    /// PTY master handle
    pub pty: Box<dyn MasterPty>,

    /// Child process handle
    pub child: Box<dyn Child>,

    /// VT parser / terminal state machine
    pub terminal: alacritty_terminal::Term<EventListener>,

    /// Scroll offset (for scrollback buffer)
    pub scroll: usize,
}
```

#### Key Dependencies

- **`portable-pty`**: Cross-platform pseudo-terminal creation. Spawns shell (bash/zsh/fish) in a PTY.
- **`alacritty_terminal`**: Full VT100/VT220/xterm terminal emulator engine from the Alacritty project. Handles all escape sequences, colors (256 + TrueColor), mouse reporting, alternate screen buffer, scrollback. This is the most battle-tested terminal emulator library in Rust.
- **`ansi-to-tui`**: Alternative lighter approach — converts raw ANSI output into Ratatui spans. Simpler but less complete than alacritty_terminal.

#### Features

| Feature | Priority | Notes |
|---------|----------|-------|
| Basic shell (PTY spawn + I/O) | P0 — MVP | portable-pty |
| Full VT escape sequence support | P0 — MVP | alacritty_terminal |
| Multiple tabs | P0 — MVP | Tab bar at top of terminal pane |
| Scrollback buffer | P1 | Configurable size |
| Copy from terminal | P1 | Mouse selection or keyboard |
| Search in terminal output | P2 | Ctrl+Shift+F in terminal |
| Split terminal panes | P3 | Horizontal/vertical splits within terminal area |
| Named/saved terminal profiles | P3 | "Build", "Test", "Server" presets |
| Click on file:line to open in editor | P2 | Regex match on terminal output |
| Broadcast input to all tabs | P3 | For multi-server commands |

---

### 3.5 `axe-lsp` — Language Server Protocol Client

LSP integration provides "intelligence" — the difference between an editor and an IDE.

#### Architecture

```rust
pub struct LspManager {
    /// Running language servers, keyed by language ID
    servers: HashMap<String, LspServer>,

    /// Configuration: which server to use for each language
    server_configs: HashMap<String, LspServerConfig>,

    /// Pending requests (awaiting response)
    pending: HashMap<RequestId, PendingRequest>,
}

pub struct LspServer {
    /// Server process
    process: Child,

    /// JSON-RPC transport (stdin/stdout)
    transport: LspTransport,

    /// Server capabilities (what this server supports)
    capabilities: ServerCapabilities,

    /// Server status
    status: LspStatus,
}

pub struct LspServerConfig {
    /// Command to start the server
    pub command: String,

    /// Arguments
    pub args: Vec<String>,

    /// File patterns this server handles
    pub file_patterns: Vec<GlobPattern>,

    /// Initialization options
    pub init_options: Option<serde_json::Value>,

    /// Settings sent after initialization
    pub settings: Option<serde_json::Value>,
}
```

#### LSP Features

| Feature | Priority | Protocol Method | Notes |
|---------|----------|----------------|-------|
| Auto-start server on file open | P0 — MVP | `initialize` | Match file extension → server config |
| Document sync | P0 — MVP | `textDocument/didOpen`, `didChange`, `didSave` | Incremental sync preferred |
| Diagnostics (errors/warnings) | P0 — MVP | `textDocument/publishDiagnostics` | Display in gutter + status bar |
| Completion (autocomplete) | P0 — MVP | `textDocument/completion` | Popup menu with fuzzy filter |
| Signature help | P1 | `textDocument/signatureHelp` | Show function params while typing |
| Go to definition | P0 — MVP | `textDocument/definition` | Ctrl+Click or hotkey |
| Go to references | P1 | `textDocument/references` | List in overlay |
| Go to implementation | P1 | `textDocument/implementation` | |
| Hover information | P1 | `textDocument/hover` | Tooltip on hover/hotkey |
| Rename symbol | P1 | `textDocument/rename` | Multi-file rename |
| Code actions (quick fixes) | P1 | `textDocument/codeAction` | Lightbulb menu |
| Format document | P1 | `textDocument/formatting` | On save (configurable) |
| Format selection | P2 | `textDocument/rangeFormatting` | |
| Document symbols | P1 | `textDocument/documentSymbol` | Symbol outline panel |
| Workspace symbols | P2 | `workspace/symbol` | Search symbols across project |
| Code lens | P3 | `textDocument/codeLens` | Inline annotations |
| Inlay hints | P2 | `textDocument/inlayHint` | Type annotations, parameter names |
| Semantic tokens | P2 | `textDocument/semanticTokens` | Enhanced highlighting from LSP |
| Call hierarchy | P3 | `textDocument/prepareCallHierarchy` | Who calls what |
| Type hierarchy | P3 | `textDocument/prepareTypeHierarchy` | Inheritance tree |

#### Pre-configured Language Servers

Default configurations shipped with Axe (user can override):

| Language | Server | Command |
|----------|--------|---------|
| Rust | rust-analyzer | `rust-analyzer` |
| Go | gopls | `gopls` |
| TypeScript/JavaScript | typescript-language-server | `typescript-language-server --stdio` |
| Python | pyright | `pyright-langserver --stdio` |
| C/C++ | clangd | `clangd` |
| Lua | lua-language-server | `lua-language-server` |
| TOML | taplo | `taplo lsp stdio` |
| YAML | yaml-language-server | `yaml-language-server --stdio` |
| JSON | vscode-json-language-server | `vscode-json-language-server --stdio` |
| HTML/CSS | vscode-html-language-server | `vscode-html-language-server --stdio` |
| Markdown | marksman | `marksman server` |
| Bash | bash-language-server | `bash-language-server start` |
| Dockerfile | docker-langserver | `docker-langserver --stdio` |
| Terraform | terraform-ls | `terraform-ls serve` |

---

### 3.6 `axe-ui` — Rendering Layer

Handles all visual output. Knows how to render, but contains no business logic.

#### Layout System

```rust
pub struct LayoutManager {
    /// Horizontal split ratio: tree panel width (percentage)
    pub tree_width_pct: u16,  // default: 20

    /// Vertical split ratio: editor vs terminal height (percentage)
    pub editor_height_pct: u16,  // default: 70

    /// Whether tree panel is visible
    pub tree_visible: bool,

    /// Whether terminal panel is visible
    pub terminal_visible: bool,

    /// Active split configuration for editor area
    pub editor_splits: SplitTree,

    /// Resize mode state (zellij-style panel resizing)
    pub resize_mode: ResizeModeState,

    /// Zoom state: if Some, the given panel is maximized
    pub zoomed_panel: Option<FocusTarget>,

    /// Saved layout before zoom (to restore on un-zoom)
    pub pre_zoom_layout: Option<SavedLayout>,
}

/// State for the interactive panel resize mode
pub struct ResizeModeState {
    /// Whether resize mode is currently active
    pub active: bool,

    /// Step size for arrow key resize (percentage points)
    pub step_small: u16,  // default: 2

    /// Step size for +/- resize (percentage points)
    pub step_large: u16,  // default: 5

    /// Minimum panel size (percentage) — prevents collapsing to zero
    pub min_size_pct: u16,  // default: 10

    /// Maximum panel size (percentage) — prevents taking over entire screen
    pub max_size_pct: u16,  // default: 90
}

/// Binary tree of editor splits
pub enum SplitTree {
    Leaf(BufferId),
    Horizontal { left: Box<SplitTree>, right: Box<SplitTree>, ratio: f32 },
    Vertical { top: Box<SplitTree>, bottom: Box<SplitTree>, ratio: f32 },
}
```

Default layout:

```
┌────────────┬──────────────────────────────────────┐
│            │  Tab Bar: [main.rs] [lib.rs] [+]     │
│  File Tree │──────────────────────────────────────│
│  (20%)     │                                      │
│            │  Editor Area (80% width, 70% height) │
│  [project] │                                      │
│   ├─ src/  │                                      │
│   ├─ test/ │                                      │
│   └─ ...   │                                      │
│            ├──────────────────────────────────────│
│            │  Terminal (30% height)                │
│            │  [tab1] [tab2] [+]                    │
│            │  $ cargo build                        │
├────────────┴──────────────────────────────────────┤
│  Status Bar: main.rs │ Rust │ Ln 42, Col 13 │ UTF-8 │ main │ 0 errors │
└───────────────────────────────────────────────────┘
```

#### Overlay Stack

Overlays render on top of the main layout and capture input focus.

```rust
pub trait Overlay {
    /// Unique identifier
    fn id(&self) -> &str;

    /// Handle input events. Return true if event was consumed.
    fn handle_event(&mut self, event: &Event) -> EventResult;

    /// Render the overlay into the given area
    fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme);

    /// Whether this overlay should dim the background
    fn dim_background(&self) -> bool { true }

    /// Size hint (percentage of screen or fixed)
    fn size_hint(&self) -> OverlaySize;
}

pub enum OverlaySize {
    /// Percentage of screen (width%, height%)
    Percentage(u16, u16),
    /// Fixed character dimensions
    Fixed(u16, u16),
    /// Floating near cursor
    AtCursor { width: u16, max_height: u16 },
}
```

Standard overlays:

| Overlay | Trigger | Description |
|---------|---------|-------------|
| File Finder | `Ctrl+P` | Fuzzy search files in project |
| Project Search | `Ctrl+Shift+F` | Search text across all files (ripgrep) |
| Command Palette | `Ctrl+Shift+P` | Search and execute any command |
| Go To Line | `Ctrl+G` | Jump to line number |
| Symbol Search | `Ctrl+Shift+O` | LSP workspace symbols |
| Buffer Switcher | `Ctrl+Tab` | Switch between open buffers |
| Completion Menu | Auto/`Ctrl+Space` | LSP completions (positioned at cursor) |
| Hover Info | `K` (normal mode) | LSP hover tooltip |
| Diagnostics List | `Ctrl+Shift+M` | All errors/warnings in project |
| Git Status | `Ctrl+Shift+G` | Modified/staged files |

#### Theme Engine

```rust
pub struct Theme {
    // --- Base Colors ---
    pub background: Color,
    pub foreground: Color,
    pub selection: Color,
    pub cursor: Color,
    pub line_highlight: Color,

    // --- UI Chrome ---
    pub panel_background: Color,
    pub panel_border: Color,
    pub panel_border_active: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_bg: Color,

    // --- Gutter ---
    pub line_number: Color,
    pub line_number_active: Color,
    pub gutter_added: Color,      // green
    pub gutter_modified: Color,   // blue
    pub gutter_deleted: Color,    // red

    // --- Diagnostics ---
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub hint: Color,

    // --- Syntax (tree-sitter scopes) ---
    pub syntax: HashMap<String, SyntaxStyle>,
}

pub struct SyntaxStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub modifiers: Modifier,  // bold, italic, underline
}
```

Syntax scope mapping (tree-sitter highlight names → theme colors):

```toml
# In theme file (TOML)
[syntax]
"keyword"          = { fg = "#c678dd", bold = true }
"keyword.control"  = { fg = "#c678dd" }
"function"         = { fg = "#61afef" }
"function.method"  = { fg = "#61afef" }
"type"             = { fg = "#e5c07b" }
"type.builtin"     = { fg = "#e5c07b", bold = true }
"variable"         = { fg = "#abb2bf" }
"variable.builtin" = { fg = "#e06c75" }
"constant"         = { fg = "#d19a66" }
"string"           = { fg = "#98c379" }
"string.escape"    = { fg = "#56b6c2" }
"comment"          = { fg = "#5c6370", italic = true }
"operator"         = { fg = "#56b6c2" }
"punctuation"      = { fg = "#abb2bf" }
"tag"              = { fg = "#e06c75" }
"attribute"        = { fg = "#d19a66" }
```

Built-in themes: `axe-dark` (default, One Dark inspired), `axe-light`, `catppuccin`, `gruvbox`, `dracula`, `tokyonight`, `nord`.

#### Styling Helper (Lip Gloss-inspired)

A fluent API for styling widgets, wrapping Ratatui primitives:

```rust
pub struct StyledBox {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    italic: bool,
    border: Option<BorderType>,
    border_fg: Option<Color>,
    padding: Padding,
    margin: Margin,
    alignment: Alignment,
    width: Option<u16>,
    height: Option<u16>,
}

impl StyledBox {
    pub fn new() -> Self { /* defaults */ }
    pub fn fg(mut self, color: impl Into<Color>) -> Self { /* ... */ }
    pub fn bg(mut self, color: impl Into<Color>) -> Self { /* ... */ }
    pub fn bold(mut self) -> Self { /* ... */ }
    pub fn padding(mut self, vertical: u16, horizontal: u16) -> Self { /* ... */ }
    pub fn border(mut self, border_type: BorderType) -> Self { /* ... */ }
    pub fn render(&self, content: &str, frame: &mut Frame, area: Rect) { /* ... */ }
}
```

#### Animations (via `tachyonfx` or custom)

Supported animation types for UI polish:

- **Fade in/out**: Overlay appearing/dismissing
- **Slide**: Panel open/close (terminal sliding up)
- **Highlight flash**: Line where cursor jumped to
- **Smooth scroll**: Scrolling through file
- **Pulse**: Notification appearance

Animations are driven by the `Tick` event at a configured FPS (default: 60).

---

### 3.7 `axe-config` — Configuration

All configuration is in TOML format, stored at `~/.config/axe/config.toml`.

#### Configuration Structure

```toml
# ~/.config/axe/config.toml

[editor]
mode = "standard"               # "standard" or "modal"
tab_size = 4
insert_spaces = true            # tabs vs spaces
word_wrap = "off"               # "off", "on", "bounded" (at column)
wrap_column = 80
show_whitespace = "selection"   # "none", "selection", "all"
cursor_blink = true
cursor_style = "block"          # "block", "line", "underline"
auto_save = false
auto_save_delay_ms = 1000
format_on_save = true
highlight_current_line = true
show_indent_guides = true
bracket_matching = true
auto_close_brackets = true
smooth_scroll = true
scroll_margin = 5               # lines to keep visible above/below cursor

[editor.minimap]
enabled = false
width = 10

[tree]
visible = true
width = 25
show_hidden = false
show_icons = true               # requires Nerd Font
sort_by = "name"                # "name", "type", "modified"
indent_size = 2

[terminal]
visible = true
height_percent = 30
shell = ""                      # empty = detect from $SHELL
scrollback_lines = 10000
copy_on_select = false

[ui]
theme = "axe-dark"
fps = 60
true_color = true
animations = true
status_bar = true
tab_bar = true
border_style = "rounded"        # "plain", "rounded", "double", "thick"

[ui.font]
# Terminal font is controlled by the terminal emulator, but
# we need to know about it for correct rendering
nerd_font = true                # enables file icons

# === Keybindings ===
# Format: "key_combo" = "command"
# Modifiers: ctrl, alt, shift, super
# Special keys: enter, esc, tab, space, backspace, delete, up, down, left, right,
#               home, end, pageup, pagedown, f1-f12

[keybindings.global]
"ctrl+shift+p" = "open_command_palette"
"ctrl+p"       = "open_file_finder"
"ctrl+shift+f" = "open_project_search"
"ctrl+s"       = "save"
"ctrl+shift+s" = "save_all"
"ctrl+q"       = "quit"
"ctrl+`"       = "toggle_terminal"
"ctrl+b"       = "toggle_tree"
"ctrl+g"       = "go_to_line"
"ctrl+tab"     = "switch_buffer"
"ctrl+alt+r"   = "enter_resize_mode"
"ctrl+alt+z"   = "zoom_panel"
"ctrl+alt+="   = "equalize_layout"

[keybindings.resize_mode]
# Active only while in resize mode (entered via ctrl+alt+r)
"left"    = "resize_left"
"right"   = "resize_right"
"up"      = "resize_up"
"down"    = "resize_down"
"+"       = "resize_grow"
"-"       = "resize_shrink"
"0"       = "equalize_layout"
"esc"     = "exit_resize_mode"
"enter"   = "exit_resize_mode"
"q"       = "exit_resize_mode"

[keybindings.editor]
"ctrl+f"       = "find"
"ctrl+h"       = "find_and_replace"
"ctrl+/"       = "toggle_comment"
"ctrl+d"       = "select_next_occurrence"
"ctrl+z"       = "undo"
"ctrl+shift+z" = "redo"
"f12"          = "go_to_definition"
"shift+f12"    = "go_to_references"
"f2"           = "rename_symbol"
"ctrl+."       = "code_action"

[keybindings.tree]
"enter"   = "open_selected"
"delete"  = "delete_selected"
"r"       = "rename_selected"
"n"       = "new_file"
"shift+n" = "new_directory"

[keybindings.terminal]
# Most keys pass through to the shell; only these are intercepted
"ctrl+shift+t" = "new_terminal_tab"
"ctrl+shift+w" = "close_terminal_tab"

# === Language Server Configuration ===
# Each entry maps a language ID to its server

[lsp.rust]
command = "rust-analyzer"
args = []
file_patterns = ["*.rs"]

[lsp.go]
command = "gopls"
args = []
file_patterns = ["*.go"]

[lsp.typescript]
command = "typescript-language-server"
args = ["--stdio"]
file_patterns = ["*.ts", "*.tsx", "*.js", "*.jsx"]

[lsp.python]
command = "pyright-langserver"
args = ["--stdio"]
file_patterns = ["*.py"]

# Override per-project in .axe/config.toml (merged with global)
```

#### Configuration Precedence

1. **Built-in defaults** (hardcoded)
2. **Global config** (`~/.config/axe/config.toml`)
3. **Project config** (`.axe/config.toml` in project root) — overrides global
4. **Command-line flags** — override everything

---

## 4. Cross-Cutting Concerns

### 4.1 Async Runtime

Axe uses **Tokio** as the async runtime. Components that need async:

- **LSP client**: JSON-RPC communication over stdin/stdout (async read/write)
- **Terminal**: Reading PTY output (continuous async stream)
- **File watcher**: Filesystem change notifications (`notify` crate, async channel)
- **Git**: Status updates in background (can be slow on large repos)

The main event loop runs synchronously (Ratatui requires this). Async tasks communicate with the main loop via `tokio::sync::mpsc` channels.

```rust
// Simplified main loop
#[tokio::main]
async fn main() {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    // Spawn async tasks
    spawn_input_reader(event_tx.clone());    // crossterm events → Event
    spawn_lsp_manager(event_tx.clone());     // LSP responses → Event
    spawn_terminal_reader(event_tx.clone()); // PTY output → Event
    spawn_file_watcher(event_tx.clone());    // FS changes → Event
    spawn_tick_timer(event_tx.clone(), fps); // periodic Tick → Event

    let mut app = AppState::new();
    let mut terminal = setup_terminal();

    loop {
        // Render current state
        terminal.draw(|frame| ui::render(&app, frame));

        // Process next event
        if let Some(event) = event_rx.recv().await {
            if app.handle_event(event).should_quit() {
                break;
            }
        }
    }

    restore_terminal();
}
```

### 4.2 Error Handling

Axe uses `anyhow` for application errors and `thiserror` for library errors in individual crates.

Non-fatal errors (LSP crash, file read failure, git error) are displayed as notifications in the status bar, never as panics. The IDE should never crash because one component failed.

```rust
pub enum Notification {
    Info(String),
    Warning(String),
    Error(String),
}
```

### 4.3 Logging

`tracing` crate with file-based subscriber. Logs go to `~/.local/share/axe/axe.log`, not to the terminal (since we own the terminal).

```rust
// Logging levels:
// TRACE: every keystroke, every render frame (development only)
// DEBUG: event dispatch, LSP messages, file operations
// INFO:  startup, shutdown, config loaded, server started
// WARN:  recoverable errors (LSP timeout, file not found)
// ERROR: unrecoverable component failures
```

### 4.4 Testing Strategy

- **Unit tests**: Each crate has isolated unit tests. Rope operations, tree-sitter parsing, keymap resolution, config parsing.
- **Integration tests**: Event flow tests — simulate keystrokes, verify state changes.
- **Snapshot tests** (via `insta` crate): Render a frame → snapshot the output → diff on changes. Catches visual regressions.
- **Fuzzing** (future): Random keystroke sequences to find panics in the editor.

---

## 5. Plugin System (P3 — Future)

### 5.1 Architecture

Plugins extend Axe without modifying core code. Two tiers:

#### Tier 1: Lua Scripting (simpler)

Embed a Lua interpreter (`mlua` crate). Plugins are `.lua` files in `~/.config/axe/plugins/`.

```lua
-- ~/.config/axe/plugins/auto_save.lua
axe.on("buffer_modified", function(event)
    axe.schedule(3000, function()  -- 3 second delay
        axe.command("save")
    end)
end)

axe.register_command("hello_world", function()
    axe.notify("Hello from plugin!")
end)

axe.keymap("ctrl+shift+h", "hello_world")
```

Plugin API exposes:
- `axe.on(event, callback)` — listen to events
- `axe.command(name)` — execute commands
- `axe.register_command(name, callback)` — register new commands
- `axe.keymap(key, command)` — bind keys
- `axe.notify(message)` — show notification
- `axe.get_buffer()` — access current buffer text
- `axe.get_cursor()` — cursor position
- `axe.insert(text)` — insert text at cursor
- `axe.open_file(path)` — open file in editor
- `axe.exec(command)` — run shell command

#### Tier 2: WASM Plugins (more powerful, sandboxed)

For heavier plugins. Compiled to WASM, loaded at runtime via `wasmtime` or `wasmer`. Sandboxed — cannot access filesystem or network unless explicitly granted.

### 5.2 Plugin Manifest

```toml
# plugin.toml
[plugin]
name = "git-blame-inline"
version = "0.1.0"
description = "Show git blame info inline"
author = "username"
license = "MIT"
engine = "lua"  # or "wasm"
entry = "init.lua"

[permissions]
filesystem = ["read"]  # read, write, or none
network = false
shell = false

[dependencies]
axe = ">=0.5.0"
```

### 5.3 Community Plugin Ideas

| Plugin | Description |
|--------|-------------|
| `git-blame` | Inline git blame annotations |
| `git-diff-view` | Side-by-side diff viewer |
| `todo-highlights` | Highlight TODO/FIXME/HACK comments |
| `color-preview` | Show color swatches for hex codes |
| `bracket-colorizer` | Rainbow brackets |
| `indent-rainbow` | Colored indentation levels |
| `project-tasks` | Parse TODO comments into a task list |
| `pomodoro` | Built-in pomodoro timer in status bar |
| `collaborative` | Real-time collaborative editing (CRDTs) |
| `ai-assist` | LLM integration (code suggestions, chat) |
| `remote-dev` | SSH remote development |
| `docker-dev` | Develop inside containers |
| `database-viewer` | Browse database tables |
| `rest-client` | HTTP request builder and sender |
| `markdown-preview` | Live preview of markdown files |
| `image-viewer` | Display images in terminal (sixel/kitty) |

---

## 6. Git Integration (P1)

Git is a first-class citizen, not a plugin.

### Features

| Feature | Priority | Implementation |
|---------|----------|---------------|
| Branch indicator in status bar | P0 — MVP | `git2` crate, read HEAD |
| Gutter diff indicators (+/-/~) | P1 | `git2` diff against HEAD |
| File tree git status icons | P1 | M (modified), A (added), ? (untracked) |
| Git status overlay | P2 | `Ctrl+Shift+G`, like VS Code source control |
| Inline blame | P2 | Per-line annotation |
| Diff viewer (side-by-side) | P2 | Split editor view |
| Commit from IDE | P3 | Commit message editor + stage/unstage |
| Git log viewer | P3 | Interactive commit history |
| Merge conflict resolver | P3 | Highlighted conflict markers with accept/reject |

### Dependency

`git2` crate (libgit2 bindings). Pure Rust alternative: `gitoxide` (`gix` crate) — faster, but less mature API.

---

## 7. Project Detection & Build Integration (P2)

Axe should detect the project type and offer appropriate commands.

### Detection

```rust
pub struct ProjectInfo {
    pub root: PathBuf,
    pub project_type: ProjectType,
    pub build_command: Option<String>,
    pub test_command: Option<String>,
    pub run_command: Option<String>,
}

pub enum ProjectType {
    Rust,       // Cargo.toml
    Go,         // go.mod
    Node,       // package.json
    Python,     // pyproject.toml, setup.py, requirements.txt
    CMake,      // CMakeLists.txt
    Make,       // Makefile
    Gradle,     // build.gradle
    Maven,      // pom.xml
    Unknown,
}
```

### Build/Run/Test Commands

Auto-detected or configured in `.axe/config.toml`:

| Project | Build | Test | Run |
|---------|-------|------|-----|
| Rust | `cargo build` | `cargo test` | `cargo run` |
| Go | `go build ./...` | `go test ./...` | `go run .` |
| Node | `npm run build` | `npm test` | `npm start` |
| Python | — | `pytest` | `python main.py` |

Hotkeys: `Ctrl+Shift+B` (build), `Ctrl+Shift+T` (test), `F5` (run).

Output goes to a dedicated terminal tab.

---

## 8. Search System (P1)

### In-File Search

Standard Ctrl+F. Highlights all matches, navigates with Enter/Shift+Enter.

- Regex support (via `regex` crate)
- Case sensitive/insensitive toggle
- Whole word toggle
- Search and replace with preview

### Project-Wide Search

Ctrl+Shift+F. Uses `grep` crate (same engine as ripgrep) for maximum performance.

```rust
pub struct ProjectSearch {
    pub query: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_patterns: Vec<GlobPattern>,
    pub exclude_patterns: Vec<GlobPattern>,
    pub results: Vec<SearchResult>,
}

pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub column: usize,
    pub line_text: String,
    pub match_range: Range<usize>,
}
```

Results displayed in an overlay with file grouping. Click or Enter to jump to the match.

---

## 9. Session Management (P2)

Save and restore IDE state across restarts.

```rust
pub struct Session {
    /// Open buffers with scroll positions and cursor locations
    pub buffers: Vec<BufferState>,

    /// Active buffer index
    pub active_buffer: usize,

    /// Layout state (panel sizes, visibility)
    pub layout: LayoutState,

    /// Terminal tabs (working directories, not shell state)
    pub terminals: Vec<TerminalSessionInfo>,

    /// File tree expanded nodes
    pub expanded_dirs: HashSet<PathBuf>,
}
```

Saved to `.axe/session.json` in the project root. Auto-restored on next open.

---

## 10. Accessibility (P2)

- **Mouse support**: Click to position cursor, click tree nodes, resize panels by dragging borders, scroll wheel.
- **Screen reader hints**: Future — structured output for terminal screen readers.
- **High contrast theme**: Built-in theme option.
- **Configurable colors**: All colors in theme files, full TrueColor support.

---

## 11. Performance Targets

| Metric | Target |
|--------|--------|
| Startup time (empty project) | < 100ms |
| Startup time (large project, 10k files) | < 500ms (lazy tree loading) |
| Keystroke-to-render latency | < 16ms (60fps) |
| File open (1MB file) | < 50ms |
| Syntax highlighting (1MB file) | < 100ms initial, < 5ms incremental |
| Project search (100k files) | < 2s |
| Memory usage (10 open buffers) | < 100MB |

---

## 12. Key Dependencies Summary

| Crate | Purpose | Used By |
|-------|---------|---------|
| `ratatui` | TUI framework (rendering) | axe-ui |
| `crossterm` | Terminal backend (input/output) | axe-ui |
| `ropey` | Rope data structure for text | axe-editor |
| `tree-sitter` | Incremental parsing / syntax highlighting | axe-editor |
| `nucleo` | Fuzzy matching (file finder, command palette) | axe-ui |
| `portable-pty` | Pseudo-terminal creation | axe-terminal |
| `alacritty_terminal` | VT terminal emulation engine | axe-terminal |
| `lsp-types` | LSP protocol type definitions | axe-lsp |
| `tokio` | Async runtime | axe-core, axe-lsp, axe-terminal |
| `serde` + `toml` | Configuration parsing | axe-config |
| `git2` or `gix` | Git integration | axe-core |
| `ignore` | .gitignore-aware file walking | axe-tree |
| `notify` | Filesystem change watching | axe-tree |
| `regex` | Search / find-and-replace | axe-editor |
| `grep` | Project-wide search (ripgrep engine) | axe-core |
| `anyhow` / `thiserror` | Error handling | all |
| `tracing` | Structured logging | all |
| `insta` | Snapshot testing | all (dev) |
| `arboard` | System clipboard access | axe-editor |
| `tachyonfx` | UI animations | axe-ui |
| `mlua` | Lua plugin scripting (future) | axe-core |
| `unicode-width` | Correct character width calculation | axe-editor, axe-ui |
| `unicode-segmentation` | Grapheme cluster handling | axe-editor |

---

## 13. Implementation Phases

### Phase 1 — Skeleton (Week 1-2)

- [ ] Cargo workspace setup with all crates
- [ ] Main event loop (Tokio + crossterm)
- [ ] Three-panel layout (tree + editor + terminal areas)
- [ ] Focus switching between panels
- [ ] Command enum + keymap resolver
- [ ] Basic theme (colors, borders)
- [ ] Status bar (static)

### Phase 2 — File Tree (Week 3-4)

- [ ] Recursive directory listing with expand/collapse
- [ ] .gitignore filtering (`ignore` crate)
- [ ] Navigation (up/down/enter)
- [ ] Open file → event to editor
- [ ] File icons (if Nerd Font detected)

### Phase 3 — Editor Core (Week 5-10)

- [ ] Rope-based buffer (ropey)
- [ ] Cursor movement (arrows, home/end, page up/down, word jump)
- [ ] Text insertion and deletion
- [ ] Line numbers in gutter
- [ ] Viewport scrolling
- [ ] Tree-sitter integration + syntax highlighting
- [ ] Basic Ctrl+F search within file
- [ ] Undo/redo
- [ ] Copy/cut/paste (system clipboard)
- [ ] Multiple buffers with tab bar
- [ ] Modified indicator (dot on tab)
- [ ] Save / Save As

### Phase 4 — Terminal (Week 11-13)

- [ ] PTY spawn with user's shell
- [ ] VT parsing (alacritty_terminal)
- [ ] Render terminal output as Ratatui widget
- [ ] Input passthrough (keys → PTY)
- [ ] Multiple terminal tabs
- [ ] Scrollback buffer

### Phase 5 — LSP (Week 14-18)

- [ ] LSP client (JSON-RPC over stdio)
- [ ] Server lifecycle (start/stop/restart)
- [ ] textDocument/didOpen + didChange + didSave
- [ ] publishDiagnostics → gutter indicators
- [ ] completion → popup menu
- [ ] definition → jump to file/line
- [ ] hover → tooltip overlay
- [ ] Format on save

### Phase 6 — Polish (Week 19-24)

- [ ] Fuzzy file finder (Ctrl+P) with nucleo
- [ ] Command palette (Ctrl+Shift+P)
- [ ] Project-wide search (Ctrl+Shift+F)
- [ ] Git status in tree + gutter + status bar
- [ ] Session save/restore
- [ ] Configuration file support
- [ ] Multiple themes
- [ ] Animations (fade, slide)
- [ ] Mouse support (click, scroll, panel resize)
- [ ] Find and replace

### Phase 7 — Advanced (Month 7+)

- [ ] Plugin system (Lua)
- [ ] Inlay hints
- [ ] Code actions
- [ ] Rename symbol
- [ ] References list
- [ ] Symbol outline panel
- [ ] Git blame / diff viewer
- [ ] Multi-cursor editing
- [ ] Debugger integration (DAP)
- [ ] Remote development (SSH)
- [ ] Collaborative editing (CRDTs)

---

## 14. File & Directory Structure

```
axe/
├── Cargo.toml                  # Workspace manifest
├── README.md
├── LICENSE
├── ARCHITECTURE.md             # This document
│
├── axe-core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── app.rs              # AppState
│       ├── event.rs            # Event enum
│       ├── command.rs          # Command enum + dispatcher
│       ├── keymap.rs           # Keymap resolver
│       ├── focus.rs            # Focus manager
│       ├── notification.rs     # Notification queue
│       ├── project.rs          # Project detection
│       └── session.rs          # Session save/restore
│
├── axe-editor/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── buffer.rs           # EditorBuffer (rope + metadata)
│       ├── buffer_manager.rs   # Multiple buffers
│       ├── cursor.rs           # Cursor state + movement
│       ├── selection.rs        # Selection handling
│       ├── history.rs          # Undo/redo tree
│       ├── highlight.rs        # Tree-sitter integration
│       ├── search.rs           # In-file search
│       ├── gutter.rs           # Line numbers + diagnostics + git
│       ├── input.rs            # Keystroke → edit operations
│       ├── mode.rs             # Modal / Standard editing modes
│       └── languages/          # Language configurations
│           ├── mod.rs
│           ├── rust.rs
│           ├── go.rs
│           └── ...
│
├── axe-tree/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── tree.rs             # FileTree structure
│       ├── node.rs             # TreeNode
│       ├── filter.rs           # Gitignore + custom filters
│       ├── icons.rs            # File type → icon mapping
│       ├── watcher.rs          # Filesystem watcher
│       └── operations.rs       # Create/delete/rename
│
├── axe-terminal/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── manager.rs          # TerminalManager
│       ├── tab.rs              # TerminalTab
│       ├── pty.rs              # PTY creation + I/O
│       ├── renderer.rs         # Terminal state → Ratatui widget
│       └── link_detector.rs    # Clickable file:line links
│
├── axe-lsp/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── manager.rs          # LspManager
│       ├── client.rs           # Single LSP server connection
│       ├── transport.rs        # JSON-RPC over stdio
│       ├── capabilities.rs     # Server capability handling
│       ├── completion.rs       # Completion logic
│       ├── diagnostics.rs      # Diagnostic processing
│       └── protocol.rs         # Request/response helpers
│
├── axe-ui/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── layout.rs           # LayoutManager + split tree
│       ├── theme.rs            # Theme engine
│       ├── styled.rs           # StyledBox (Lip Gloss-like API)
│       ├── status_bar.rs       # Bottom status bar
│       ├── tab_bar.rs          # Buffer tabs
│       ├── overlays/
│       │   ├── mod.rs
│       │   ├── file_finder.rs  # Ctrl+P
│       │   ├── command_palette.rs  # Ctrl+Shift+P
│       │   ├── project_search.rs   # Ctrl+Shift+F
│       │   ├── go_to_line.rs
│       │   ├── completion.rs   # LSP autocomplete popup
│       │   ├── hover.rs        # LSP hover tooltip
│       │   └── dialog.rs       # Confirmation dialogs
│       ├── widgets/
│       │   ├── mod.rs
│       │   ├── editor_view.rs  # Editor rendering widget
│       │   ├── tree_view.rs    # File tree rendering widget
│       │   ├── terminal_view.rs # Terminal rendering widget
│       │   ├── gradient.rs     # Gradient text rendering
│       │   └── animation.rs    # Animation helpers
│       └── render.rs           # Main render function
│
├── axe-config/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── config.rs           # AppConfig struct
│       ├── keybindings.rs      # Keybinding parsing
│       ├── theme_loader.rs     # Theme file loading
│       ├── lsp_config.rs       # LSP server configs
│       └── defaults.rs         # Built-in defaults
│
├── themes/
│   ├── axe-dark.toml
│   ├── axe-light.toml
│   ├── catppuccin.toml
│   ├── gruvbox.toml
│   └── ...
│
├── queries/                    # Tree-sitter query files
│   ├── rust/
│   │   ├── highlights.scm
│   │   ├── injections.scm
│   │   └── indents.scm
│   ├── go/
│   ├── javascript/
│   ├── typescript/
│   ├── python/
│   └── ...
│
└── scripts/
    ├── install.sh              # Installation script
    └── fetch-grammars.sh       # Download tree-sitter grammars
```

---

## 15. Design Decisions & Rationale

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Language | Rust | Go (Charmbracelet) | Tree-sitter native bindings, ropey, alacritty_terminal, performance |
| TUI Framework | Ratatui | tui-rs, cursive, Bubble Tea (Go) | Active development, diff rendering, constraint layout, largest ecosystem |
| Terminal Backend | Crossterm | Termion, ncurses | Cross-platform (Windows), pure Rust, active maintenance |
| Text Storage | Rope (ropey) | Gap Buffer, Piece Table | O(log n) operations, proven in Helix, memory efficient |
| Syntax Engine | Tree-sitter | regex-based (syntect), LSP semantic tokens | Incremental parsing, structural queries (folding, textobjects, indents), 100+ grammars |
| Terminal Emulator | alacritty_terminal | vte + custom grid, raw ANSI parsing | Battle-tested, full xterm compatibility, TrueColor, mouse support |
| Fuzzy Matcher | nucleo | skim, fzf bindings | Fastest in benchmarks, async, used by Helix |
| Async Runtime | Tokio | async-std, smol | Ecosystem dominance, LSP libraries expect it |
| Config Format | TOML | YAML, JSON, Lua | Rust ecosystem standard, human-friendly, typed parsing with serde |
| VCS Integration | git2 | gitoxide (gix), shelling out to git | Mature, full-featured, in-process (no subprocess overhead) |
| Plugin Scripting | Lua (mlua) | Rhai, WASM-only, JavaScript | Lua is proven for editor scripting (Neovim), lightweight, fast |

---

## 16. Naming

**Axe** — short, sharp, powerful. A tool for shaping raw material into something useful. Three letters, instant to type, impossible to forget. `$ axe .` and you're in.

Binary name: `axe`
Config directory: `~/.config/axe/`
Data directory: `~/.local/share/axe/`
Project-local config: `.axe/`

---

*This document should be included as context when continuing development of Axe in future conversations.*
