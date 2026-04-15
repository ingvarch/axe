// IMPACT ANALYSIS — app::ai_overlay dispatch
// Parents: execute.rs dispatches Command::{ToggleAiOverlay,SelectAiAgent,KillAiSession}
//          to these AppState methods. input.rs calls `start_ai_session` after the
//          first-run picker returns a selection.
// Children: core AiOverlay state (crate::ai_overlay::AiOverlay), which wraps a
//          TerminalTab for the actual PTY. axe-config for reading/writing the
//          chosen default to ~/.config/axe/config.toml.
// Siblings: ConfirmDialog is reused for the "kill current X session?" prompt.
//          The picker overlay reuses AgentPicker from the core ai_overlay module.
// Risk: spawning a PTY is fallible. Failures surface as status messages; the
//       overlay stays hidden rather than half-constructed.

use std::path::PathBuf;

use super::AppState;
use crate::ai_overlay::detect::{detect_available, WhichResolver};
use crate::ai_overlay::registry::{merged_agents, ResolvedAgent};
use crate::ai_overlay::AgentPicker;

impl AppState {
    /// Handles `Command::ToggleAiOverlay`.
    ///
    /// State machine:
    /// - visible → hide, leave session alone.
    /// - hidden, session alive → show.
    /// - hidden, session dead → drop it and fall through to respawn.
    /// - hidden, no session + saved default that resolves on PATH → spawn it.
    /// - hidden, no session + no valid default → open the first-run picker.
    pub fn toggle_ai_overlay(&mut self) {
        if self.ai_overlay.visible {
            self.ai_overlay.visible = false;
            self.reset_ai_overlay_selection_state();
            return;
        }

        // Visible == false below this point.
        if self.ai_overlay.session.is_some() && !self.ai_overlay.session_is_alive() {
            self.ai_overlay.kill_session();
        }

        if self.ai_overlay.session.is_some() {
            self.ai_overlay.visible = true;
            return;
        }

        // No session — need to figure out what to spawn.
        let all = merged_agents(&self.config.ai.agents);
        let available = detect_available(&all, &WhichResolver);

        if available.is_empty() {
            self.set_status_message(
                "No AI agents found in PATH. Install claude/codex/gemini/qwen/aider to use the AI overlay.".to_string(),
            );
            return;
        }

        // If we already have a saved default AND it still resolves, spawn it.
        if let Some(default_id) = self.config.ai.default.clone() {
            if let Some(agent) = available.iter().find(|a| a.id == default_id).cloned() {
                self.start_ai_session(&agent);
                return;
            }
        }

        // First-run OR stale default: show the picker. If only one agent is
        // available, skip the picker and auto-pick it (per the approved spec).
        if available.len() == 1 {
            let agent = available.into_iter().next().expect("len == 1");
            self.save_ai_default(&agent.id);
            self.start_ai_session(&agent);
            return;
        }

        self.ai_overlay.picker = Some(AgentPicker::new(available));
        self.ai_overlay.visible = true;
    }

    /// Handles `Command::SelectAiAgent`.
    ///
    /// Opens the agent picker independently of the toggle hotkey — used from
    /// the command palette to let the user change agents without first hiding
    /// the overlay. The picker overlays whatever state the overlay is in.
    pub fn select_ai_agent(&mut self) {
        let all = merged_agents(&self.config.ai.agents);
        let available = detect_available(&all, &WhichResolver);

        if available.is_empty() {
            self.set_status_message("No AI agents found in PATH.".to_string());
            return;
        }

        self.ai_overlay.picker = Some(AgentPicker::new(available));
        self.ai_overlay.visible = true;
    }

    /// Handles `Command::KillAiSession`. Drops the current session (killing
    /// its child process) and hides the overlay.
    pub fn kill_ai_session(&mut self) {
        self.ai_overlay.kill_session();
        self.ai_overlay.visible = false;
        self.reset_ai_overlay_selection_state();
    }

    /// Drops any pending mouse-selection state so the next time the overlay
    /// opens it starts with a clean slate — no stale `selecting` flag, no
    /// prior click count affecting multi-click detection, no leftover
    /// highlighted range in the underlying PTY grid.
    fn reset_ai_overlay_selection_state(&mut self) {
        self.ai_overlay_selecting = false;
        self.ai_overlay_select_start = None;
        self.ai_overlay_click_state = super::types::ClickState::default();
        if let Some(session) = self.ai_overlay.session.as_mut() {
            session.tab.clear_selection();
        }
    }

    /// Spawns a PTY session for `agent` and saves it as the new default.
    ///
    /// If a session is already running for a different agent, it is killed
    /// first (the caller is expected to have shown a confirmation dialog).
    pub(crate) fn start_ai_session(&mut self, agent: &ResolvedAgent) {
        let cwd = self.ai_session_cwd();
        match self.ai_overlay.start_session(agent, &cwd) {
            Ok(()) => {
                self.ai_overlay.visible = true;
                self.save_ai_default(&agent.id);
            }
            Err(e) => {
                log::warn!("Failed to start AI session '{}': {e:#}", agent.id);
                self.set_status_message(format!("AI agent '{}' failed to start: {e}", agent.id));
                self.ai_overlay.visible = false;
            }
        }
    }

    /// Persists the given agent id as the new default in the global config,
    /// and mirrors it into the in-memory config so the next toggle uses it.
    fn save_ai_default(&mut self, agent_id: &str) {
        self.config.ai.default = Some(agent_id.to_string());

        let path = match self.ai_config_path_override.clone() {
            Some(p) => p,
            None => match global_config_path() {
                Some(p) => p,
                None => {
                    log::warn!("Cannot determine config dir; AI default not persisted");
                    return;
                }
            },
        };

        if let Err(e) = axe_config::AppConfig::save_ai_section(&self.config.ai, &path) {
            log::warn!("Failed to save AI default to {}: {e:#}", path.display());
            self.set_status_message(format!("Failed to save AI default: {e}"));
        }
    }

    /// Working directory to use when spawning an AI agent.
    ///
    /// Prefers the project root (if any), falls back to the process cwd, and
    /// finally to the user's home dir so spawning cannot fail on a missing dir.
    fn ai_session_cwd(&self) -> PathBuf {
        self.project_root
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Returns the path to the global config file, or `None` if no config dir
/// can be determined on this platform.
fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("axe").join("config.toml"))
}
