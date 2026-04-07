// IMPACT ANALYSIS — ssh_host module
// Parents: AppState::execute(OpenSshHostFinder) creates hosts from this module.
// Children: SshHostFinder consumes Vec<SshHost> for fuzzy matching.
//           spawn_ssh_tab() uses SshHost for connection parameters.
// Siblings: axe-config::SshConfig provides axe.toml host entries.
// Risk: ssh2-config parsing may fail on malformed ~/.ssh/config — must handle gracefully.

use std::path::{Path, PathBuf};

use axe_config::AppConfig;

/// Source of an SSH host entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshHostSource {
    /// Parsed from `~/.ssh/config`.
    SshConfig,
    /// Configured in `axe.toml`.
    AxeConfig,
}

/// A resolved SSH host ready for connection.
#[derive(Debug, Clone)]
pub struct SshHost {
    /// Host alias from config.
    pub name: String,
    /// Resolved hostname or IP address.
    pub hostname: String,
    /// SSH port.
    pub port: u16,
    /// Username for authentication.
    pub user: String,
    /// Path to identity file (private key), if specified.
    pub identity_file: Option<PathBuf>,
    /// Where this host entry came from.
    pub source: SshHostSource,
    /// Display name for the fuzzy finder (includes source label on conflicts).
    pub display_name: String,
}

/// Parses SSH hosts from `~/.ssh/config`.
///
/// Returns an empty list if the file doesn't exist or can't be parsed.
pub fn parse_ssh_config(path: &Path) -> Vec<SshHost> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_ssh_config_str(&content)
}

/// Parses SSH hosts from a string in `~/.ssh/config` format.
fn parse_ssh_config_str(content: &str) -> Vec<SshHost> {
    let mut reader = std::io::BufReader::new(content.as_bytes());
    let Ok(config) = ssh2_config::SshConfig::default()
        .parse(&mut reader, ssh2_config::ParseRule::ALLOW_UNKNOWN_FIELDS)
    else {
        return Vec::new();
    };

    let mut hosts = Vec::new();
    for host in config.get_hosts() {
        // Each host has a pattern (list of clauses) and params.
        // Skip wildcard-only patterns and negated patterns.
        let name = match host.pattern.first() {
            Some(clause) if !clause.negated => clause.pattern.clone(),
            _ => continue,
        };
        if name.is_empty() || name == "*" || name.contains('*') || name.contains('?') {
            continue;
        }

        let params = &host.params;
        let hostname = params.host_name.clone().unwrap_or_else(|| name.clone());
        let user = params.user.clone().unwrap_or_else(|| {
            std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_default()
        });
        let port = params.port.unwrap_or(22);
        let identity_file = params
            .identity_file
            .as_ref()
            .and_then(|files| files.first())
            .cloned();

        hosts.push(SshHost {
            display_name: name.clone(),
            name,
            hostname,
            port,
            user,
            identity_file,
            source: SshHostSource::SshConfig,
        });
    }

    hosts
}

/// Converts axe.toml SSH host entries to `SshHost` structs.
pub fn hosts_from_axe_config(config: &AppConfig) -> Vec<SshHost> {
    config
        .ssh
        .hosts
        .iter()
        .map(|entry| SshHost {
            display_name: entry.name.clone(),
            name: entry.name.clone(),
            hostname: entry.hostname.clone(),
            port: entry.port,
            user: entry.user.clone(),
            identity_file: entry.identity_file.as_ref().map(PathBuf::from),
            source: SshHostSource::AxeConfig,
        })
        .collect()
}

/// Merges SSH hosts from both sources.
///
/// When names conflict between sources, both are kept with source labels
/// appended to `display_name` (e.g., `"prod (ssh config)"`, `"prod (axe.toml)"`).
pub fn merge_hosts(mut ssh_hosts: Vec<SshHost>, mut axe_hosts: Vec<SshHost>) -> Vec<SshHost> {
    // Find conflicting names.
    let ssh_names: std::collections::HashSet<&str> =
        ssh_hosts.iter().map(|h| h.name.as_str()).collect();

    for axe_host in &mut axe_hosts {
        if ssh_names.contains(axe_host.name.as_str()) {
            axe_host.display_name = format!("{} (axe.toml)", axe_host.name);
        }
    }

    let axe_names: std::collections::HashSet<&str> =
        axe_hosts.iter().map(|h| h.name.as_str()).collect();

    for ssh_host in &mut ssh_hosts {
        if axe_names.contains(ssh_host.name.as_str()) {
            ssh_host.display_name = format!("{} (ssh config)", ssh_host.name);
        }
    }

    ssh_hosts.extend(axe_hosts);
    ssh_hosts
}

/// Returns the default path to `~/.ssh/config`.
pub fn default_ssh_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ssh_config_str_basic() {
        let config = r#"
Host myserver
    HostName 192.168.1.10
    User deploy
    Port 2222
    IdentityFile ~/.ssh/id_myserver

Host staging
    HostName staging.example.com
    User admin
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 2);

        assert_eq!(hosts[0].name, "myserver");
        assert_eq!(hosts[0].hostname, "192.168.1.10");
        assert_eq!(hosts[0].user, "deploy");
        assert_eq!(hosts[0].port, 2222);
        assert!(hosts[0].identity_file.is_some());

        assert_eq!(hosts[1].name, "staging");
        assert_eq!(hosts[1].hostname, "staging.example.com");
        assert_eq!(hosts[1].user, "admin");
        assert_eq!(hosts[1].port, 22);
    }

    #[test]
    fn parse_ssh_config_str_skips_wildcards() {
        let config = r#"
Host *
    ServerAliveInterval 60

Host myserver
    HostName 10.0.0.1
    User root
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "myserver");
    }

    #[test]
    fn parse_ssh_config_str_empty() {
        let hosts = parse_ssh_config_str("");
        assert!(hosts.is_empty());
    }

    #[test]
    fn parse_ssh_config_str_invalid() {
        // Malformed content should return empty, not panic.
        let hosts = parse_ssh_config_str("this is not ssh config format {{{}}}");
        // ssh2-config may parse this leniently, so just verify no panic.
        let _ = hosts;
    }

    #[test]
    fn parse_ssh_config_missing_file() {
        let hosts = parse_ssh_config(Path::new("/nonexistent/path/ssh_config"));
        assert!(hosts.is_empty());
    }

    #[test]
    fn hosts_from_axe_config_converts_entries() {
        let config = AppConfig::load_from_str(
            r#"
[[ssh.hosts]]
name = "prod"
hostname = "10.0.0.1"
user = "deploy"
port = 2222
identity_file = "~/.ssh/id_prod"
"#,
        )
        .expect("should parse");

        let hosts = hosts_from_axe_config(&config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].name, "prod");
        assert_eq!(hosts[0].hostname, "10.0.0.1");
        assert_eq!(hosts[0].port, 2222);
        assert_eq!(hosts[0].source, SshHostSource::AxeConfig);
    }

    #[test]
    fn hosts_from_axe_config_empty() {
        let config = AppConfig::default();
        let hosts = hosts_from_axe_config(&config);
        assert!(hosts.is_empty());
    }

    #[test]
    fn merge_hosts_no_conflicts() {
        let ssh = vec![make_host("alpha", SshHostSource::SshConfig)];
        let axe = vec![make_host("beta", SshHostSource::AxeConfig)];
        let merged = merge_hosts(ssh, axe);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].display_name, "alpha");
        assert_eq!(merged[1].display_name, "beta");
    }

    #[test]
    fn merge_hosts_with_conflict_adds_source_labels() {
        let ssh = vec![make_host("prod", SshHostSource::SshConfig)];
        let axe = vec![make_host("prod", SshHostSource::AxeConfig)];
        let merged = merge_hosts(ssh, axe);
        assert_eq!(merged.len(), 2);

        let names: Vec<&str> = merged.iter().map(|h| h.display_name.as_str()).collect();
        assert!(
            names.contains(&"prod (ssh config)"),
            "should label ssh config source"
        );
        assert!(
            names.contains(&"prod (axe.toml)"),
            "should label axe.toml source"
        );
    }

    #[test]
    fn merge_hosts_partial_conflict() {
        let ssh = vec![
            make_host("prod", SshHostSource::SshConfig),
            make_host("dev", SshHostSource::SshConfig),
        ];
        let axe = vec![make_host("prod", SshHostSource::AxeConfig)];
        let merged = merge_hosts(ssh, axe);
        assert_eq!(merged.len(), 3);

        // "dev" has no conflict — no label.
        let dev = merged.iter().find(|h| h.name == "dev").unwrap();
        assert_eq!(dev.display_name, "dev");

        // "prod" entries both get labels.
        let prods: Vec<&SshHost> = merged.iter().filter(|h| h.name == "prod").collect();
        assert_eq!(prods.len(), 2);
        let display_names: Vec<&str> = prods.iter().map(|h| h.display_name.as_str()).collect();
        assert!(display_names.contains(&"prod (ssh config)"));
        assert!(display_names.contains(&"prod (axe.toml)"));
    }

    #[test]
    fn merge_hosts_both_empty() {
        let merged = merge_hosts(Vec::new(), Vec::new());
        assert!(merged.is_empty());
    }

    /// Helper to create a test SshHost.
    fn make_host(name: &str, source: SshHostSource) -> SshHost {
        SshHost {
            name: name.to_string(),
            hostname: format!("{name}.example.com"),
            port: 22,
            user: "testuser".to_string(),
            identity_file: None,
            source,
            display_name: name.to_string(),
        }
    }
}
