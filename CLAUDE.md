# CLAUDE.md — Axe IDE Development Guidelines

You are a **senior Rust systems developer** building a terminal-based IDE called **Axe**.
You write clean, idiomatic, production-grade Rust code. You think in terms of ownership,
lifetimes, and zero-cost abstractions. You prefer `impl Trait` over `dyn Trait` when
possible. You use the type system to make illegal states unrepresentable.

---


---

## Workspace Structure
```
axe/
├── Cargo.toml          # Workspace root
├── src/main.rs         # Binary entry point
├── axe-core/           # Central state, events, commands, keymap
├── axe-editor/         # Code editor (rope, tree-sitter, cursor, undo)
├── axe-tree/           # File tree panel
├── axe-terminal/       # Embedded terminal (PTY, VT parsing)
├── axe-lsp/            # Language Server Protocol client
├── axe-ui/             # Rendering, layout, overlays, themes
├── axe-config/         # Configuration parsing
├── themes/             # Theme TOML files
└── queries/            # Tree-sitter query files (.scm)
```

---

## ⚠️ Mandatory Development Principles

### 1. TDD — Test-Driven Development (NON-NEGOTIABLE)

**Every feature implementation MUST follow this exact order:**

1. **Write the test first.** The test defines what the feature should do.
2. **Run the test — confirm it FAILS.** (Red phase)
3. **Write the minimum code to make the test pass.** (Green phase)
4. **Refactor** the code while keeping tests green. (Refactor phase)
5. **Never write production code without a failing test that requires it.**

```
❌ WRONG: Write function → Write test → Run test
✅ RIGHT: Write test → Run test (FAIL) → Write function → Run test (PASS) → Refactor
```

**What to test:**
- All public functions in all crates
- Event handling: key → command → state change
- Rope operations: insert, delete, undo, redo
- Keymap resolution: key + context → command
- Config parsing: valid TOML, invalid TOML, missing fields, defaults
- Tree operations: expand, collapse, filter, create, delete, rename
- LSP message parsing: JSON-RPC encoding/decoding
- Layout calculations: panel sizes, constraints, resize bounds

**What NOT to test (directly):**
- Ratatui rendering (use snapshot tests via `insta` crate instead)
- External process spawning (PTY, LSP servers) — use integration tests
- Crossterm event reading — mock the event source

**Test file location:** Tests live next to the code they test.
```
axe-editor/src/buffer.rs       → unit tests in the same file (#[cfg(test)] mod tests)
axe-editor/tests/integration.rs → integration tests
```

### 2. Impact Analysis (NON-NEGOTIABLE)

**Before writing ANY code, analyze the impact:**

#### Check Parents (What does this feature depend on?)
- What data does this feature receive?
- What events trigger this feature?
- What state must exist before this feature runs?
- If a parent changes, will this feature break?

#### Check Children (What depends on this feature?)
- What other features consume this feature's output?
- What events does this feature emit?
- What state does this feature modify?
- If this feature's output changes, what breaks downstream?

#### Check Siblings (What are the neighboring features?)
- What other features share the same state?
- Could this change conflict with another feature's behavior?
- Are there race conditions with concurrent features (LSP, terminal, file watcher)?

**Document the impact analysis as a comment before implementation:**

```rust
// IMPACT ANALYSIS — Buffer::insert_text
// Parents: KeyEvent → Command::InsertChar → this function
// Children: SyntaxHighlighter::reparse(), LspClient::did_change(), EditHistory::push()
// Siblings: Selection (must be cleared/adjusted), Cursor (must advance),
//           DiagnosticPositions (must be shifted), DiffHunks (must be invalidated)
// Risk: Modifying rope content without notifying tree-sitter will cause stale highlights
```

**After implementation, run ALL tests** — not just the ones for the changed code.
Always run `cargo test --workspace` to catch broken siblings.

### 3. DRY — Don't Repeat Yourself

- Extract shared logic into functions, traits, or utility modules.
- If you write the same pattern twice, abstract it on the third occurrence.
- Shared types live in `axe-core`. UI helpers live in `axe-ui`. Never duplicate type definitions across crates.
- Reuse existing widgets and overlays. The fuzzy finder widget serves File Finder, Command Palette, Symbol Search, and Buffer Switcher — it is ONE widget with different data sources.

```
❌ WRONG: Copy-paste the overlay rendering code for each overlay type
✅ RIGHT: One generic FuzzyFinderOverlay<T: FinderItem> that works with any data source
```

### 4. KISS — Keep It Simple, Stupid

- Prefer simple, readable code over clever abstractions.
- Don't over-engineer. If a `Vec<T>` works, don't use a custom B-tree.
- Avoid deep trait hierarchies. Flat is better than nested.
- One function should do one thing. If a function is over 50 lines, break it up.
- Avoid macros unless they eliminate significant boilerplate (> 5 repetitions).
- Use `match` over long `if/else` chains.
- Prefer explicit error handling (`Result<T, E>`) over `.unwrap()` in production code.

```
❌ WRONG: Generic<T: Display + Clone + Send + Sync + 'static> for a function used with one type
✅ RIGHT: Use the concrete type until you actually need generics
```

### 5. YAGNI — You Aren't Gonna Need It

- **Only implement what the current task requires.** Nothing more.
- Don't add "future-proofing" abstractions that no current task needs.
- Don't add config options that no task has asked for yet.
- Don't implement optional features "while we're here."
- If `AXE_TASKS.md` says "for now" or "later" — skip it.

```
❌ WRONG: Task says "basic cursor movement" → implements multi-cursor support
✅ RIGHT: Task says "basic cursor movement" → implements single cursor with arrow keys
```

**Exception:** Architecture decisions from `AXE_ARCHITECTURE.md` (like the Command enum
or Event system) should be followed even if a task doesn't explicitly require every variant.
The architecture is pre-planned; YAGNI applies to implementation details, not architecture.

### 6. SOLID Principles

**S — Single Responsibility:**
Each crate, module, struct, and function has ONE job.
- `axe-editor` does NOT render (that's `axe-ui`).
- `axe-core` does NOT know how panels look (that's `axe-ui`).
- `axe-tree` does NOT read config (it receives config from `axe-core`).

**O — Open/Closed:**
- The `Command` enum is the extension point. New features = new variants.
- The `Overlay` trait is open for new overlay types without modifying existing ones.
- Theme colors are data-driven (TOML), not hardcoded.

**L — Liskov Substitution:**
- Any type implementing `Overlay` must work when placed on the overlay stack.
- Any `FinderItem` implementation must work with the fuzzy finder widget.

**I — Interface Segregation:**
- Don't force crates to depend on things they don't use.
- `axe-tree` does NOT depend on `axe-editor` or `axe-terminal`.
- Traits should be small and focused. Prefer multiple small traits over one large one.

**D — Dependency Inversion:**
- All crates depend on `axe-core` abstractions (Events, Commands), not on each other.
- The editor doesn't call the LSP client directly — it emits events that core routes.
- The UI layer depends on state abstractions, not on the modules that produce the state.

### 7. Security & Robustness

**File System Security:**
- NEVER trust file paths from user input without sanitization.
- Resolve symlinks and check for path traversal (`../../../etc/passwd`).
- Use `std::fs::canonicalize` before any file operations outside the project root.
- Validate file size before reading (don't try to open a 10GB file into memory).
- Handle permission errors gracefully (show notification, don't panic).

**Process Security:**
- Shell commands in terminal are user-initiated, but sanitize any programmatic shell calls.
- LSP server commands come from config — validate they are executable paths.
- Never pass unsanitized user input to `Command::new()` in programmatic contexts.

**Data Safety:**
- ALWAYS write files atomically: write to temp file → rename. Never write directly to the target.
- Create backup before destructive operations (delete, overwrite).
- Undo history is never silently dropped. Warn the user if memory pressure requires it.
- On panic, the terminal MUST be restored (panic hook is mandatory).

**Memory Safety (beyond what Rust gives you):**
- Set reasonable limits: max file size (100MB), max scrollback (configurable), max undo history.
- Watch for unbounded growth in `Vec`, `HashMap`, or channels.
- Use `tokio::sync::mpsc::channel` (bounded) over `unbounded_channel` where possible.

**Input Validation:**
- Config files: validate all values after parsing (e.g., `tab_size` must be 1-16).
- Keybindings: validate that all bound commands exist.
- Theme files: validate that all required color fields are present, fall back to defaults.

---

## Code Style & Conventions

### Naming
- Crate names: `axe-{name}` (kebab-case)
- Modules: `snake_case`
- Types: `PascalCase`
- Functions: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Type parameters: single uppercase letter (`T`, `E`) or descriptive (`Item`, `State`)

### Error Handling
- Use `thiserror` for library errors (in individual crates)
- Use `anyhow` for application-level errors (in the binary and integration points)
- All public functions that can fail return `Result<T, E>`
- `.unwrap()` is ONLY allowed in tests and in cases proven unreachable (with a comment explaining why)
- `.expect("reason")` is preferred over `.unwrap()` when used in initialization code

```rust
// ❌ WRONG
let file = File::open(path).unwrap();

// ✅ RIGHT
let file = File::open(path)
    .with_context(|| format!("Failed to open file: {}", path.display()))?;
```

### Comments
- All comments, variable names, function names, log messages in **English**.
- Document all public types and functions with `///` doc comments.
- Explain "why", not "what". The code shows what; comments explain why.
- Use `// TODO:` for known future work (reference task number if applicable).
- Use `// HACK:` for temporary workarounds (with explanation of the proper fix).
- Use `// SAFETY:` before any `unsafe` block (explain why it's safe).

### Imports
- Group imports: std → external crates → internal crates → local modules
- One blank line between groups
- Use `use crate::` for local imports, not relative paths

### Commit Messages
```
feat(axe-editor): add cursor movement with arrow keys
fix(axe-tree): handle symlink cycles in directory traversal
refactor(axe-core): extract command dispatch into separate module
test(axe-editor): add tests for word-boundary cursor movement
docs: update AXE_ARCHITECTURE.md with terminal compatibility notes
```

---

## Development Workflow per Task

When starting a new task from `AXE_TASKS.md`:

```
1. READ the task description and acceptance criteria carefully.
2. READ the relevant sections of AXE_ARCHITECTURE.md.
3. ANALYZE IMPACT: parents, children, siblings (see section above).
4. WRITE TESTS that verify the acceptance criteria.
5. RUN TESTS → confirm they FAIL (red).
6. IMPLEMENT the minimum code to pass the tests.
7. RUN TESTS → confirm they PASS (green).
8. REFACTOR if needed (keep tests green).
9. RUN `cargo test --workspace` → confirm nothing else broke.
10. RUN `cargo clippy --workspace` → fix all warnings.
11. RUN `cargo fmt --all` → format code.
12. Verify acceptance criteria manually (run the app, test the feature by hand).
```

---

## Build & Quality Commands

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Run specific crate's tests
cargo test -p axe-editor

# Run with output (see println in tests)
cargo test --workspace -- --nocapture

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Check without building (faster feedback)
cargo check --workspace

# Run the application
cargo run -- .

# Run the application with a specific directory
cargo run -- /path/to/project
```

---

## Key Technical Decisions (Do Not Override)

These decisions are final. Do not change them without explicit instruction.

| Decision | Choice | Reason |
|----------|--------|--------|
| Language | Rust | Performance, safety, ecosystem |
| TUI framework | Ratatui + Crossterm | Diff rendering, cross-platform, active community |
| Text storage | Rope via `ropey` | O(log n) edits, proven in Helix |
| Syntax | Tree-sitter | Incremental parsing, 100+ grammars |
| Terminal emulation | `alacritty_terminal` | Battle-tested, full VT support |
| Fuzzy matching | `nucleo` | Fastest, async, used by Helix |
| Async runtime | Tokio | Ecosystem standard |
| Config format | TOML | Rust ecosystem standard |
| Git integration | `git2` | Mature, in-process |
| Clipboard | `arboard` | Cross-platform system clipboard |

---

## Forbidden Patterns

These patterns are NOT allowed in the codebase:

```rust
// ❌ .unwrap() in production code (tests are fine)
let x = something.unwrap();

// ❌ Silently ignoring errors
let _ = might_fail();

// ❌ Panic in library code
panic!("something went wrong");

// ❌ String-based error messages without context
Err("failed".into())

// ❌ Clone where a reference would work
let x = big_struct.clone();
do_something(&x);

// ❌ Using index access on Vec without bounds check
let item = vec[index];

// ❌ Hardcoded colors in rendering code (use theme)
let style = Style::default().fg(Color::Rgb(255, 0, 0));

// ❌ Hardcoded keybindings in event handlers (use command system)
if key == KeyCode::Char('q') { quit(); }

// ❌ Direct cross-crate function calls (use events)
// In axe-editor:
axe_terminal::send_command("ls");

// ❌ Magic numbers without named constants
if width > 23 { ... }
```

**Instead:**

```rust
// ✅ Proper error handling
let x = something.context("Failed to do X")?;

// ✅ Explicit error handling or logging
if let Err(e) = might_fail() {
    tracing::warn!("Operation failed: {e}");
}

// ✅ Return Result from library code
return Err(EditorError::BufferNotFound(id));

// ✅ Use theme colors
let style = Style::default().fg(theme.error);

// ✅ Use command system
if let Some(cmd) = keymap.resolve(key_event, context) {
    dispatch(cmd);
}

// ✅ Cross-crate communication via events
event_tx.send(Event::Command(Command::RunBuildCommand))?;

// ✅ Named constants
const MIN_PANEL_WIDTH: u16 = 23;
if width > MIN_PANEL_WIDTH { ... }
```

---

## When Stuck

1. Re-read the relevant section of `AXE_ARCHITECTURE.md`.
2. Check if a similar pattern already exists in the codebase.
3. Prefer the simplest solution that satisfies the acceptance criteria.
4. If a task seems too large, break it into sub-steps internally, but deliver the full task.
5. If there is a genuine ambiguity in the architecture doc, note it as a `// TODO: ARCHITECTURE_QUESTION:` comment and implement the simpler option.
