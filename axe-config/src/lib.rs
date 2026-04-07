pub mod theme;

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

/// Application configuration loaded from TOML files.
///
/// All fields have sensible defaults via `#[serde(default)]`, so an empty
/// or partial TOML file is always valid.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub tree: TreeConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub ui: UiConfig,
    /// SSH remote host configurations.
    #[serde(default)]
    pub ssh: SshConfig,
    /// User-configured keybinding overrides.
    ///
    /// Maps key combo strings (e.g., `"ctrl+q"`) to command names
    /// (e.g., `"request_quit"`). Applied on top of default bindings.
    #[serde(default)]
    pub keybindings: HashMap<String, String>,
    /// LSP server configurations, keyed by language ID.
    ///
    /// Example TOML: `[lsp.rust]` with `command = "rust-analyzer"`.
    /// User entries override built-in defaults from `default_lsp_configs()`.
    #[serde(default)]
    pub lsp: HashMap<String, LspServerConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        // Deserialize empty TOML to get serde defaults.
        toml::from_str("").expect("empty TOML should always deserialize")
    }
}

/// Editor-related configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "default_tab_size")]
    pub tab_size: usize,
    #[serde(default = "default_true")]
    pub insert_spaces: bool,
    #[serde(default = "default_true")]
    pub highlight_current_line: bool,
    #[serde(default = "default_scroll_margin")]
    pub scroll_margin: usize,
    #[serde(default)]
    pub auto_save: bool,
    #[serde(default = "default_auto_save_delay_ms")]
    pub auto_save_delay_ms: u64,
    #[serde(default)]
    pub word_wrap: bool,
    #[serde(default)]
    pub format_on_save: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: default_tab_size(),
            insert_spaces: true,
            highlight_current_line: true,
            scroll_margin: default_scroll_margin(),
            auto_save: false,
            auto_save_delay_ms: default_auto_save_delay_ms(),
            word_wrap: false,
            format_on_save: false,
        }
    }
}

/// File tree panel configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TreeConfig {
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_tree_width")]
    pub width: u16,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default = "default_true")]
    pub show_icons: bool,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            visible: true,
            width: default_tree_width(),
            show_hidden: false,
            show_icons: true,
            sort_order: default_sort_order(),
        }
    }
}

/// Terminal panel configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TerminalConfig {
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_terminal_height_pct")]
    pub height_percent: u16,
    #[serde(default)]
    pub shell: String,
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            visible: true,
            height_percent: default_terminal_height_pct(),
            shell: String::new(),
            scrollback_lines: default_scrollback_lines(),
        }
    }
}

/// UI configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme_name")]
    pub theme: String,
    #[serde(default = "default_border_style")]
    pub border_style: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme_name(),
            border_style: default_border_style(),
        }
    }
}

/// SSH remote host configurations.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SshConfig {
    /// Configured SSH hosts.
    #[serde(default)]
    pub hosts: Vec<SshHostEntry>,
}

/// Configuration for a single SSH host.
#[derive(Debug, Clone, Deserialize)]
pub struct SshHostEntry {
    /// Display name / alias for the host.
    pub name: String,
    /// Hostname or IP address.
    pub hostname: String,
    /// SSH port (default: 22).
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    /// Username for authentication.
    pub user: String,
    /// Path to identity file (private key).
    #[serde(default)]
    pub identity_file: Option<String>,
}

/// Configuration for a single LSP server.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LspServerConfig {
    /// Command to spawn the LSP server (e.g., `"rust-analyzer"`).
    pub command: String,
    /// Arguments to pass to the server command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional initialization options sent to the server.
    #[serde(default)]
    pub init_options: Option<serde_json::Value>,
}

/// Returns built-in LSP server configurations for common languages.
///
/// These are used as defaults and can be overridden by user config.
pub fn default_lsp_configs() -> HashMap<String, LspServerConfig> {
    let mut configs = HashMap::new();

    configs.insert(
        "rust".to_string(),
        LspServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec![],
            init_options: None,
        },
    );
    configs.insert(
        "go".to_string(),
        LspServerConfig {
            command: "gopls".to_string(),
            args: vec![],
            init_options: None,
        },
    );
    configs.insert(
        "python".to_string(),
        LspServerConfig {
            command: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            init_options: None,
        },
    );
    configs.insert(
        "typescript".to_string(),
        LspServerConfig {
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            init_options: None,
        },
    );
    configs.insert(
        "javascript".to_string(),
        LspServerConfig {
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            init_options: None,
        },
    );
    configs.insert(
        "c".to_string(),
        LspServerConfig {
            command: "clangd".to_string(),
            args: vec![],
            init_options: None,
        },
    );
    configs.insert(
        "cpp".to_string(),
        LspServerConfig {
            command: "clangd".to_string(),
            args: vec![],
            init_options: None,
        },
    );
    configs.insert(
        "lua".to_string(),
        LspServerConfig {
            command: "lua-language-server".to_string(),
            args: vec![],
            init_options: None,
        },
    );
    configs.insert(
        "toml".to_string(),
        LspServerConfig {
            command: "taplo".to_string(),
            args: vec!["lsp".to_string(), "stdio".to_string()],
            init_options: None,
        },
    );
    configs.insert(
        "terraform".to_string(),
        LspServerConfig {
            command: "terraform-ls".to_string(),
            args: vec!["serve".to_string()],
            init_options: None,
        },
    );
    configs.insert(
        "shellscript".to_string(),
        LspServerConfig {
            command: "bash-language-server".to_string(),
            args: vec!["start".to_string()],
            init_options: None,
        },
    );

    configs
}

// --- Default value functions ---

fn default_tab_size() -> usize {
    4
}
fn default_true() -> bool {
    true
}
fn default_scroll_margin() -> usize {
    5
}
fn default_auto_save_delay_ms() -> u64 {
    1000
}
fn default_tree_width() -> u16 {
    25
}
fn default_terminal_height_pct() -> u16 {
    30
}
fn default_scrollback_lines() -> usize {
    10000
}
fn default_theme_name() -> String {
    "axe-dark".to_string()
}
fn default_border_style() -> String {
    "rounded".to_string()
}
fn default_sort_order() -> String {
    "directories_first".to_string()
}
fn default_ssh_port() -> u16 {
    22
}

impl AppConfig {
    /// Loads configuration from global and project-level config files.
    ///
    /// Search order:
    /// 1. `~/.config/axe/config.toml` (global)
    /// 2. `{project_root}/.axe/config.toml` (project, overrides global)
    ///
    /// If no config files exist, returns defaults. Malformed files are logged
    /// and skipped.
    pub fn load(project_root: Option<&Path>) -> Self {
        let (config, _warnings) = Self::load_with_warnings(project_root);
        config
    }

    /// Loads configuration and collects warnings for malformed config files.
    ///
    /// Returns `(config, warnings)` where `warnings` contains human-readable
    /// messages about config files that failed to parse. The config will use
    /// defaults for any sections that could not be loaded.
    pub fn load_with_warnings(project_root: Option<&Path>) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();

        // Load global config from ~/.config/axe/config.toml.
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(".config").join("axe").join("config.toml");
            match Self::load_from_path(&global_path) {
                Ok(Some(global)) => config = global,
                Ok(None) => {} // File doesn't exist, no warning needed.
                Err(msg) => warnings.push(msg),
            }
        }

        // Overlay project config.
        if let Some(root) = project_root {
            let project_path = root.join(".axe").join("config.toml");
            match Self::load_from_path(&project_path) {
                Ok(Some(project)) => config.merge(project),
                Ok(None) => {} // File doesn't exist, no warning needed.
                Err(msg) => warnings.push(msg),
            }
        }

        (config, warnings)
    }

    /// Parses config from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the TOML string cannot be parsed.
    pub fn load_from_str(toml_str: &str) -> Result<Self> {
        let config: Self = toml::from_str(toml_str)?;
        Ok(config)
    }

    /// Loads config from a file path.
    ///
    /// Returns `Ok(Some(config))` on success, `Ok(None)` if the file doesn't
    /// exist, or `Err(warning_message)` if the file exists but is malformed.
    fn load_from_path(path: &Path) -> std::result::Result<Option<Self>, String> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Ok(None),
            Err(_) => return Ok(None),
        };
        match toml::from_str(&content) {
            Ok(config) => Ok(Some(config)),
            Err(e) => {
                let msg = format!("Failed to parse config {}: {e}", path.display());
                log::warn!("{msg}");
                Err(msg)
            }
        }
    }

    /// Merges another config on top of this one.
    ///
    /// The `other` config's values take precedence. This is used for
    /// project-level overrides on top of global config.
    fn merge(&mut self, other: Self) {
        // For simplicity, project config completely overrides each section.
        // This is the right behavior: if a project config specifies [editor],
        // all editor settings come from the project config.
        *self = other;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = AppConfig::default();
        assert_eq!(config.editor.tab_size, 4);
        assert!(config.editor.insert_spaces);
        assert!(config.editor.highlight_current_line);
        assert_eq!(config.editor.scroll_margin, 5);
        assert!(!config.editor.auto_save);
        assert_eq!(config.editor.auto_save_delay_ms, 1000);
        assert!(!config.editor.word_wrap);
        assert!(!config.editor.format_on_save);
        assert!(config.tree.visible);
        assert_eq!(config.tree.width, 25);
        assert!(!config.tree.show_hidden);
        assert!(config.tree.show_icons);
        assert_eq!(config.tree.sort_order, "directories_first");
        assert!(config.terminal.visible);
        assert_eq!(config.terminal.height_percent, 30);
        assert!(config.terminal.shell.is_empty());
        assert_eq!(config.terminal.scrollback_lines, 10000);
        assert_eq!(config.ui.theme, "axe-dark");
        assert_eq!(config.ui.border_style, "rounded");
        assert!(config.keybindings.is_empty());
    }

    #[test]
    fn parse_empty_toml_returns_defaults() {
        let config = AppConfig::load_from_str("").expect("empty TOML should parse");
        assert_eq!(config.editor.tab_size, 4);
        assert_eq!(config.ui.theme, "axe-dark");
        assert!(config.keybindings.is_empty());
    }

    #[test]
    fn parse_partial_toml_overrides_specified_fields() {
        let toml_str = r#"
[editor]
tab_size = 2
auto_save = true

[ui]
theme = "axe-light"
"#;
        let config = AppConfig::load_from_str(toml_str).expect("partial TOML should parse");
        assert_eq!(config.editor.tab_size, 2);
        assert!(config.editor.auto_save);
        // Unspecified fields keep defaults
        assert!(config.editor.insert_spaces);
        assert_eq!(config.ui.theme, "axe-light");
        assert_eq!(config.ui.border_style, "rounded");
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let result = AppConfig::load_from_str("this is not valid [[[");
        assert!(result.is_err());
    }

    #[test]
    fn parse_unknown_fields_are_ignored() {
        let toml_str = r#"
[editor]
tab_size = 8
unknown_field = "ignored"

[unknown_section]
foo = "bar"
"#;
        // This should not error - unknown fields are silently ignored
        let config = AppConfig::load_from_str(toml_str);
        assert!(config.is_ok());
        assert_eq!(
            config
                .expect("should parse with unknown fields")
                .editor
                .tab_size,
            8
        );
    }

    #[test]
    fn load_with_no_config_files_returns_defaults() {
        // Use a temp dir as project root - no .axe/ dir exists
        let dir = tempfile::tempdir().expect("should create temp dir");
        let config = AppConfig::load(Some(dir.path()));
        assert_eq!(config.editor.tab_size, 4);
        assert_eq!(config.ui.theme, "axe-dark");
    }

    #[test]
    fn load_project_config_from_file() {
        let dir = tempfile::tempdir().expect("should create temp dir");
        let axe_dir = dir.path().join(".axe");
        std::fs::create_dir_all(&axe_dir).expect("should create .axe dir");
        std::fs::write(
            axe_dir.join("config.toml"),
            r#"
[editor]
tab_size = 2
"#,
        )
        .expect("should write config file");
        let config = AppConfig::load(Some(dir.path()));
        assert_eq!(config.editor.tab_size, 2);
    }

    #[test]
    fn merge_project_overrides_global() {
        let mut global = AppConfig::load_from_str(
            r#"
[editor]
tab_size = 4
auto_save = true
"#,
        )
        .expect("should parse global config");
        let project = AppConfig::load_from_str(
            r#"
[editor]
tab_size = 2
"#,
        )
        .expect("should parse project config");
        global.merge(project);
        assert_eq!(global.editor.tab_size, 2);
    }

    #[test]
    fn config_keybindings_parsed_from_toml() {
        let toml = r#"
[keybindings]
"ctrl+q" = "toggle_tree"
"alt+x" = "save"
"#;
        let config = AppConfig::load_from_str(toml).expect("should parse keybindings");
        assert_eq!(config.keybindings.len(), 2);
        assert_eq!(
            config.keybindings.get("ctrl+q").map(String::as_str),
            Some("toggle_tree")
        );
    }

    #[test]
    fn default_config_has_no_keybindings() {
        let config = AppConfig::default();
        assert!(config.keybindings.is_empty());
    }

    // --- Part A: New config fields ---

    #[test]
    fn default_config_has_word_wrap_false() {
        let config = AppConfig::default();
        assert!(!config.editor.word_wrap);
    }

    #[test]
    fn default_config_has_format_on_save_false() {
        let config = AppConfig::default();
        assert!(!config.editor.format_on_save);
    }

    #[test]
    fn default_config_has_sort_order_directories_first() {
        let config = AppConfig::default();
        assert_eq!(config.tree.sort_order, "directories_first");
    }

    #[test]
    fn parse_word_wrap_true() {
        let toml_str = r#"
[editor]
word_wrap = true
"#;
        let config = AppConfig::load_from_str(toml_str).expect("should parse");
        assert!(config.editor.word_wrap);
    }

    #[test]
    fn parse_format_on_save_true() {
        let toml_str = r#"
[editor]
format_on_save = true
"#;
        let config = AppConfig::load_from_str(toml_str).expect("should parse");
        assert!(config.editor.format_on_save);
    }

    // --- Part B: Config error notification ---

    #[test]
    fn load_with_warnings_valid_config_no_warnings() {
        let dir = tempfile::tempdir().expect("should create temp dir");
        let axe_dir = dir.path().join(".axe");
        std::fs::create_dir_all(&axe_dir).expect("should create .axe dir");
        std::fs::write(
            axe_dir.join("config.toml"),
            r#"
[editor]
tab_size = 2
"#,
        )
        .expect("should write config file");
        let (config, warnings) = AppConfig::load_with_warnings(Some(dir.path()));
        assert_eq!(config.editor.tab_size, 2);
        assert!(warnings.is_empty());
    }

    #[test]
    fn load_with_warnings_invalid_config_returns_defaults_and_warning() {
        let dir = tempfile::tempdir().expect("should create temp dir");
        let axe_dir = dir.path().join(".axe");
        std::fs::create_dir_all(&axe_dir).expect("should create .axe dir");
        std::fs::write(axe_dir.join("config.toml"), "this is [[[not valid toml")
            .expect("should write config file");
        let (config, warnings) = AppConfig::load_with_warnings(Some(dir.path()));
        // Should fall back to defaults
        assert_eq!(config.editor.tab_size, 4);
        assert_eq!(config.ui.theme, "axe-dark");
        // Should have a warning about the parse error
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Failed to parse config"));
    }

    #[test]
    fn load_with_warnings_no_config_no_warnings() {
        let dir = tempfile::tempdir().expect("should create temp dir");
        let (config, warnings) = AppConfig::load_with_warnings(Some(dir.path()));
        assert_eq!(config.editor.tab_size, 4);
        assert!(warnings.is_empty());
    }

    // --- Part C: LSP config ---

    #[test]
    fn default_config_has_empty_lsp() {
        let config = AppConfig::default();
        assert!(config.lsp.is_empty());
    }

    #[test]
    fn parse_lsp_config_from_toml() {
        let toml_str = r#"
[lsp.rust]
command = "rust-analyzer"

[lsp.python]
command = "pyright-langserver"
args = ["--stdio"]
"#;
        let config = AppConfig::load_from_str(toml_str).expect("should parse LSP config");
        assert_eq!(config.lsp.len(), 2);
        assert_eq!(config.lsp["rust"].command, "rust-analyzer");
        assert_eq!(config.lsp["python"].command, "pyright-langserver");
        assert_eq!(config.lsp["python"].args, vec!["--stdio"]);
    }

    #[test]
    fn default_lsp_configs_has_common_servers() {
        let configs = default_lsp_configs();
        assert!(configs.contains_key("rust"));
        assert!(configs.contains_key("go"));
        assert!(configs.contains_key("python"));
        assert!(configs.contains_key("typescript"));
        assert!(configs.contains_key("c"));
        assert!(configs.contains_key("lua"));
        assert!(configs.contains_key("toml"));
        assert!(configs.contains_key("shellscript"));
        assert!(configs.contains_key("terraform"));
        assert_eq!(configs["rust"].command, "rust-analyzer");
        assert_eq!(configs["go"].command, "gopls");
        assert_eq!(configs["terraform"].command, "terraform-ls");
        assert_eq!(configs["terraform"].args, vec!["serve"]);
    }

    #[test]
    fn lsp_server_config_default() {
        let config = LspServerConfig::default();
        assert!(config.command.is_empty());
        assert!(config.args.is_empty());
        assert!(config.init_options.is_none());
    }

    // --- SSH config ---

    #[test]
    fn default_config_has_empty_ssh_hosts() {
        let config = AppConfig::default();
        assert!(config.ssh.hosts.is_empty());
    }

    #[test]
    fn parse_ssh_hosts_from_toml() {
        let toml_str = r#"
[[ssh.hosts]]
name = "prod"
hostname = "192.168.1.10"
user = "deploy"
port = 2222
identity_file = "~/.ssh/id_prod"

[[ssh.hosts]]
name = "staging"
hostname = "staging.example.com"
user = "admin"
"#;
        let config = AppConfig::load_from_str(toml_str).expect("should parse SSH config");
        assert_eq!(config.ssh.hosts.len(), 2);
        assert_eq!(config.ssh.hosts[0].name, "prod");
        assert_eq!(config.ssh.hosts[0].hostname, "192.168.1.10");
        assert_eq!(config.ssh.hosts[0].user, "deploy");
        assert_eq!(config.ssh.hosts[0].port, 2222);
        assert_eq!(
            config.ssh.hosts[0].identity_file.as_deref(),
            Some("~/.ssh/id_prod")
        );
        assert_eq!(config.ssh.hosts[1].name, "staging");
        assert_eq!(config.ssh.hosts[1].port, 22); // default
        assert!(config.ssh.hosts[1].identity_file.is_none());
    }

    #[test]
    fn parse_ssh_empty_section_returns_defaults() {
        let toml_str = r#"
[ssh]
"#;
        let config = AppConfig::load_from_str(toml_str).expect("should parse empty SSH section");
        assert!(config.ssh.hosts.is_empty());
    }
}
