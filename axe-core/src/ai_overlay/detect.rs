// IMPACT ANALYSIS — ai_overlay::detect
// Parents: merged_agents(&config.ai.agents) → the full candidate list.
// Children: filtered list feeds into the first-run picker and is also used to
//           validate that a saved default still resolves on PATH.
// Siblings: execute.rs calls detect_available on every ToggleAiOverlay so it
//           can fall back to the picker if the saved default has disappeared.
// Risk: hitting the filesystem on every toggle; mitigated by `which` being
//       tiny and by not caching stale results (users do install new CLIs).

use std::path::PathBuf;

use crate::ai_overlay::registry::ResolvedAgent;

/// Resolves a command name to an absolute path on disk.
///
/// This abstraction exists purely for testing — the production implementation
/// wraps the `which` crate, while tests inject a [`FakeResolver`] to get
/// deterministic "agent is installed" answers without touching `$PATH`.
pub trait BinaryResolver {
    fn resolve(&self, command: &str) -> Option<PathBuf>;
}

/// Production resolver backed by the `which` crate.
pub struct WhichResolver;

impl BinaryResolver for WhichResolver {
    fn resolve(&self, command: &str) -> Option<PathBuf> {
        which::which(command).ok()
    }
}

/// Returns the subset of `agents` whose `command` resolves on `$PATH` (or is an
/// absolute path that exists and is executable).
///
/// Order is preserved from the input list.
pub fn detect_available(
    agents: &[ResolvedAgent],
    resolver: &dyn BinaryResolver,
) -> Vec<ResolvedAgent> {
    agents
        .iter()
        .filter(|a| resolver.resolve(&a.command).is_some())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Test resolver that returns `Some(path)` for any command in its allow-list.
    struct FakeResolver {
        allowed: HashSet<String>,
    }

    impl FakeResolver {
        fn new(allowed: &[&str]) -> Self {
            Self {
                allowed: allowed.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl BinaryResolver for FakeResolver {
        fn resolve(&self, command: &str) -> Option<PathBuf> {
            if self.allowed.contains(command) {
                Some(PathBuf::from(format!("/fake/bin/{command}")))
            } else {
                None
            }
        }
    }

    fn agent(id: &str, command: &str) -> ResolvedAgent {
        ResolvedAgent {
            id: id.to_string(),
            command: command.to_string(),
            args: Vec::new(),
            display: id.to_string(),
        }
    }

    #[test]
    fn all_agents_found() {
        let agents = vec![agent("claude", "claude"), agent("gemini", "gemini")];
        let resolver = FakeResolver::new(&["claude", "gemini"]);

        let found = detect_available(&agents, &resolver);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "claude");
        assert_eq!(found[1].id, "gemini");
    }

    #[test]
    fn no_agents_found() {
        let agents = vec![agent("claude", "claude"), agent("gemini", "gemini")];
        let resolver = FakeResolver::new(&[]);

        let found = detect_available(&agents, &resolver);
        assert!(found.is_empty());
    }

    #[test]
    fn partial_agents_found_preserves_order() {
        let agents = vec![
            agent("claude", "claude"),
            agent("codex", "codex"),
            agent("gemini", "gemini"),
            agent("aider", "aider"),
        ];
        let resolver = FakeResolver::new(&["codex", "aider"]);

        let found = detect_available(&agents, &resolver);
        let ids: Vec<&str> = found.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["codex", "aider"]);
    }

    #[test]
    fn user_agent_with_absolute_path_detected() {
        let agents = vec![agent("custom", "/opt/bin/custom")];
        let resolver = FakeResolver::new(&["/opt/bin/custom"]);

        let found = detect_available(&agents, &resolver);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "custom");
    }
}
