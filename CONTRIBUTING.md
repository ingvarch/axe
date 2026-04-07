# Contributing to Axe

Thank you for your interest in contributing to Axe. This guide covers everything
you need to get started.

## Prerequisites

- **Rust 1.75+** -- install via [rustup](https://rustup.rs/)
- **cargo**, **clippy**, **rustfmt** -- included with rustup
- **A C compiler** -- required for tree-sitter grammars (gcc, clang, or MSVC)
- **Git** -- for version control and conventional commits

## Getting Started

```bash
# Clone the repository
git clone https://github.com/ingvarch/axe.git
cd axe

# Build the entire workspace
cargo build --workspace

# Run tests to verify everything works
cargo test --workspace

# Run the application
cargo run -- .
```

## Development Workflow

**Before every commit**, run the full check suite:

```bash
make check
```

This runs, in order:

1. `cargo fmt --all -- --check` -- formatting
2. `cargo clippy --workspace --all-targets -- -D warnings` -- linting
3. `cargo test --workspace` -- all tests

All three must pass. Do not commit code that fails any of these checks.

You can also run each step individually:

```bash
make fmt      # Check formatting
make clippy   # Run linter
make test     # Run tests
make build    # Build release binary
```

## Code Style

The full code style guide lives in `CLAUDE.md`. Here are the key rules:

- **No `.unwrap()` in production code.** Use `anyhow::Result` with `.context()` for
  application-level errors and `thiserror` for library-level errors.
- **Doc comments on all public APIs.** Use `///` doc comments on every public type,
  function, and method.
- **English everywhere.** All code, comments, variable names, log messages, and
  documentation must be in English.
- **No hardcoded colors.** Use the theme system. No `Color::Rgb(...)` literals in
  rendering code.
- **No hardcoded keybindings.** Use the command and keymap system.
- **Named constants over magic numbers.** Define constants instead of using bare
  numeric literals.
- **Import grouping.** Order: std, external crates, internal crates, local modules.
  Separate each group with a blank line.

## Testing

This project follows **Test-Driven Development (TDD)**. The workflow is:

1. Write a failing test that describes the expected behavior.
2. Confirm it fails for the right reason.
3. Write the minimum code to make the test pass.
4. Refactor while keeping the test green.

### Where tests live

Unit tests live next to the code they test, inside `#[cfg(test)]` modules:

```rust
// In axe-editor/src/buffer.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_text_at_position() {
        // ...
    }
}
```

Integration tests live in `<crate>/tests/` directories.

### Running tests

```bash
cargo test --workspace              # All tests
cargo test -p axe-editor            # Single crate
cargo test --workspace -- --nocapture  # With stdout output
```

Always run `cargo test --workspace` before submitting a PR to catch regressions
across crates.

## Pull Request Process

1. **Keep PRs focused.** One feature, one fix, or one refactor per PR. Small PRs
   are easier to review and merge.
2. **Describe what and why.** The PR description should explain what changed and
   the reasoning behind it.
3. **All CI checks must pass.** Formatting, clippy, and tests are non-negotiable.
4. **Include tests.** New features and bug fixes must include corresponding tests.
5. **Perform impact analysis.** Before changing code, trace the call chain upward
   (what calls this?) and downward (what does this call?). Check sibling modules
   at the same layer for unintended side effects.

## Architecture

Axe is a 7-crate Cargo workspace. Each crate has a single responsibility:

| Crate | Purpose |
|-------|---------|
| `axe-core` | Central state, events, commands, keymap |
| `axe-editor` | Code editor (rope, tree-sitter, cursor, undo) |
| `axe-tree` | File tree panel |
| `axe-terminal` | Embedded terminal (PTY, VT parsing) |
| `axe-lsp` | Language Server Protocol client |
| `axe-ui` | Rendering, layout, overlays, themes |
| `axe-config` | Configuration parsing |

Crates communicate through the event and command system defined in `axe-core`.
Direct cross-crate function calls are not allowed -- use events instead.

For detailed design decisions and data flow, see `AXE_ARCHITECTURE.md`.

## Commit Message Format

This project uses [Conventional Commits](https://www.conventionalcommits.org/).
Every commit message must follow this format:

```
<type>(<scope>): <description>
```

**Types:**

| Type | Use for |
|------|---------|
| `feat` | New features |
| `fix` | Bug fixes |
| `refactor` | Code restructuring without behavior change |
| `test` | Adding or updating tests |
| `docs` | Documentation changes |
| `chore` | Build system, CI, dependencies |

**Scopes** match crate names: `axe-core`, `axe-editor`, `axe-tree`, `axe-terminal`,
`axe-lsp`, `axe-ui`, `axe-config`. Omit scope for cross-cutting changes.

**Examples:**

```
feat(axe-editor): add word-boundary cursor movement
fix(axe-tree): handle symlink cycles in directory traversal
refactor(axe-core): extract command dispatch into separate module
test(axe-lsp): add JSON-RPC encoding round-trip tests
docs: update CONTRIBUTING.md with testing guidelines
```

Use imperative mood in the description ("add", not "added" or "adds").

## License

By contributing to Axe, you agree that your contributions will be licensed under
the MIT License.
