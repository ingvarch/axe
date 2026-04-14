// IMPACT ANALYSIS — ai_overlay::registry
// Parents: axe-config AiConfig provides user-defined agents that override/extend
//          the built-in list. Callers pass `&ai.agents` into merged_agents().
// Children: ResolvedAgent is consumed by detect (PATH filtering), picker (UI),
//           and execute.rs when spawning the PTY for a chosen agent.
// Siblings: AiAgentConfig in axe-config mirrors the user TOML schema — keep the
//           fields in sync with that struct.

use std::collections::HashMap;

use axe_config::AiAgentConfig;

/// Built-in AI agent entry known to Axe out of the box.
///
/// User config can override any of these by id or add new ones via `[ai.agents.*]`.
pub struct BuiltinAgent {
    pub id: &'static str,
    pub command: &'static str,
    pub display: &'static str,
}

/// Built-in registry of AI coding agents known as of 2026.
///
/// Order matters: first-run auto-pick prefers entries earlier in this list when
/// multiple are detected. Claude Code leads because it is currently the most
/// widely used terminal-native agent.
pub const BUILTIN_AGENTS: &[BuiltinAgent] = &[
    BuiltinAgent {
        id: "claude",
        command: "claude",
        display: "Claude Code",
    },
    BuiltinAgent {
        id: "codex",
        command: "codex",
        display: "Codex CLI",
    },
    BuiltinAgent {
        id: "gemini",
        command: "gemini",
        display: "Gemini CLI",
    },
    BuiltinAgent {
        id: "qwen",
        command: "qwen",
        display: "Qwen Code",
    },
    BuiltinAgent {
        id: "aider",
        command: "aider",
        display: "Aider",
    },
    BuiltinAgent {
        id: "opencode",
        command: "opencode",
        display: "OpenCode",
    },
    BuiltinAgent {
        id: "goose",
        command: "goose",
        display: "Goose",
    },
];

/// A fully-resolved AI agent — after merging built-ins with user config.
///
/// Everything downstream (detector, picker, spawner) works with this type so it
/// no longer has to care whether the entry came from the hardcoded registry or
/// from the user's `[ai.agents.*]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgent {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub display: String,
}

/// Merges built-in agents with user-defined ones into a single ordered list.
///
/// Rules:
/// - Built-ins come first, in the order defined by [`BUILTIN_AGENTS`].
/// - If a user entry has the same id as a built-in, it overrides (same slot).
/// - User entries with new ids are appended after all built-ins, sorted by id
///   so that the order is deterministic across runs.
pub fn merged_agents(user: &HashMap<String, AiAgentConfig>) -> Vec<ResolvedAgent> {
    let mut out: Vec<ResolvedAgent> = Vec::with_capacity(BUILTIN_AGENTS.len() + user.len());

    for builtin in BUILTIN_AGENTS {
        if let Some(overrider) = user.get(builtin.id) {
            out.push(ResolvedAgent {
                id: builtin.id.to_string(),
                command: overrider.command.clone(),
                args: overrider.args.clone(),
                display: overrider.display_name.clone(),
            });
        } else {
            out.push(ResolvedAgent {
                id: builtin.id.to_string(),
                command: builtin.command.to_string(),
                args: Vec::new(),
                display: builtin.display.to_string(),
            });
        }
    }

    let builtin_ids: std::collections::HashSet<&str> =
        BUILTIN_AGENTS.iter().map(|b| b.id).collect();
    let mut extra_keys: Vec<&String> = user
        .keys()
        .filter(|k| !builtin_ids.contains(k.as_str()))
        .collect();
    extra_keys.sort();
    for key in extra_keys {
        let entry = &user[key];
        out.push(ResolvedAgent {
            id: key.clone(),
            command: entry.command.clone(),
            args: entry.args.clone(),
            display: entry.display_name.clone(),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_agent(command: &str, display: &str) -> AiAgentConfig {
        AiAgentConfig {
            command: command.to_string(),
            args: Vec::new(),
            display_name: display.to_string(),
        }
    }

    #[test]
    fn merged_without_user_returns_all_builtins_in_order() {
        let agents = merged_agents(&HashMap::new());
        assert_eq!(agents.len(), BUILTIN_AGENTS.len());
        for (i, builtin) in BUILTIN_AGENTS.iter().enumerate() {
            assert_eq!(agents[i].id, builtin.id);
            assert_eq!(agents[i].command, builtin.command);
            assert_eq!(agents[i].display, builtin.display);
            assert!(agents[i].args.is_empty());
        }
    }

    #[test]
    fn user_overrides_builtin_by_id_keeps_position() {
        let mut user = HashMap::new();
        user.insert(
            "claude".to_string(),
            AiAgentConfig {
                command: "/opt/bin/claude-dev".to_string(),
                args: vec!["--model".to_string(), "opus".to_string()],
                display_name: "Claude (dev)".to_string(),
            },
        );

        let agents = merged_agents(&user);
        assert_eq!(agents.len(), BUILTIN_AGENTS.len());
        // Claude is still at index 0.
        assert_eq!(agents[0].id, "claude");
        assert_eq!(agents[0].command, "/opt/bin/claude-dev");
        assert_eq!(
            agents[0].args,
            vec!["--model".to_string(), "opus".to_string()]
        );
        assert_eq!(agents[0].display, "Claude (dev)");
    }

    #[test]
    fn user_adds_new_id_appended_after_builtins() {
        let mut user = HashMap::new();
        user.insert("my-agent".to_string(), mk_agent("/opt/my", "My Agent"));

        let agents = merged_agents(&user);
        assert_eq!(agents.len(), BUILTIN_AGENTS.len() + 1);
        let last = agents.last().unwrap();
        assert_eq!(last.id, "my-agent");
        assert_eq!(last.command, "/opt/my");
        assert_eq!(last.display, "My Agent");
    }

    #[test]
    fn multiple_user_extras_are_sorted_by_id() {
        let mut user = HashMap::new();
        user.insert("zebra".to_string(), mk_agent("z", "Zebra"));
        user.insert("alpha".to_string(), mk_agent("a", "Alpha"));
        user.insert("mango".to_string(), mk_agent("m", "Mango"));

        let agents = merged_agents(&user);
        let extras: Vec<&str> = agents
            .iter()
            .skip(BUILTIN_AGENTS.len())
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(extras, vec!["alpha", "mango", "zebra"]);
    }

    #[test]
    fn merged_order_is_stable_across_calls() {
        let mut user = HashMap::new();
        user.insert("xxx".to_string(), mk_agent("x", "X"));
        user.insert("yyy".to_string(), mk_agent("y", "Y"));

        let a = merged_agents(&user);
        let b = merged_agents(&user);
        assert_eq!(a, b);
    }
}
