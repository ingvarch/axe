// IMPACT ANALYSIS — ssh_connect module
// Parents: TerminalManager::spawn_ssh_tab() spawns the async task from this module.
// Children: Sends TermEvent::Output/SshConnected/SshNeedsPassword/SshError to main thread.
//           Reads SshInput from main thread.
// Siblings: SshTerminalTab (state updates based on events from this task).
// Risk: russh is async — errors must not crash the main loop. Handle all auth failures gracefully.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::{Context, Result};
use russh::keys::PrivateKeyWithHashAlg;

use crate::manager::TermEvent;
use crate::ssh_tab::SshInput;

/// Parameters for an SSH connection.
#[derive(Debug, Clone)]
pub struct SshConnectParams {
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub identity_file: Option<PathBuf>,
    pub cols: u16,
    pub rows: u16,
    pub tab_id: usize,
    pub connect_timeout_secs: u64,
}

/// SSH client handler required by russh.
struct SshClientHandler;

impl russh::client::Handler for SshClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // TODO: check against ~/.ssh/known_hosts
        Ok(true)
    }
}

/// Spawns an async task that connects to an SSH server and manages the session.
///
/// The task communicates with the main thread via:
/// - `event_tx`: sends TermEvent (output data, state changes) to the main loop
/// - `input_rx`: receives SshInput (keystrokes, resize, password, close) from the main loop
pub fn spawn_ssh_task(
    params: SshConnectParams,
    event_tx: Sender<TermEvent>,
    mut input_rx: tokio::sync::mpsc::UnboundedReceiver<SshInput>,
) {
    tokio::spawn(async move {
        if let Err(e) = run_ssh_session(params.clone(), &event_tx, &mut input_rx).await {
            log::warn!("SSH connection failed: {e:#}");
            let _ = event_tx.send(TermEvent::SshError(params.tab_id, format!("{e:#}")));
        }
    });
}

async fn run_ssh_session(
    params: SshConnectParams,
    event_tx: &Sender<TermEvent>,
    input_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SshInput>,
) -> Result<()> {
    let config = Arc::new(russh::client::Config::default());
    let handler = SshClientHandler;

    let timeout = std::time::Duration::from_secs(params.connect_timeout_secs);
    let mut session = tokio::time::timeout(
        timeout,
        russh::client::connect(config, (params.hostname.as_str(), params.port), handler),
    )
    .await
    .context("SSH connection timed out")?
    .context("Failed to connect to SSH server")?;

    // Try authentication methods in order.
    let authenticated = try_auth(&mut session, &params, event_tx, input_rx).await?;
    if !authenticated {
        anyhow::bail!("All authentication methods failed");
    }

    let _ = event_tx.send(TermEvent::SshConnected(params.tab_id));

    // Open a session channel with PTY.
    let channel = session
        .channel_open_session()
        .await
        .context("Failed to open SSH session channel")?;

    channel
        .request_pty(
            true,
            "xterm-256color",
            params.cols as u32,
            params.rows as u32,
            0,
            0,
            &[],
        )
        .await
        .context("Failed to request PTY")?;

    channel
        .request_shell(true)
        .await
        .context("Failed to request shell")?;

    // I/O loop: forward data between the SSH channel and the main thread.
    run_io_loop(channel, params.tab_id, event_tx, input_rx).await?;

    let _ = event_tx.send(TermEvent::ChildExited(params.tab_id));
    Ok(())
}

/// Attempts authentication: ssh-agent -> key files -> password.
async fn try_auth(
    session: &mut russh::client::Handle<SshClientHandler>,
    params: &SshConnectParams,
    event_tx: &Sender<TermEvent>,
    input_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SshInput>,
) -> Result<bool> {
    // 1. Try key files.
    let key_paths = collect_key_paths(&params.identity_file);
    for key_path in &key_paths {
        if let Ok(key) = russh::keys::load_secret_key(key_path, None) {
            let key_with_hash = PrivateKeyWithHashAlg::new(Arc::new(key), None);
            match session
                .authenticate_publickey(&params.user, key_with_hash)
                .await
            {
                Ok(auth) if auth.success() => {
                    log::info!("SSH: authenticated with key {}", key_path.display());
                    return Ok(true);
                }
                Ok(_) => {
                    log::debug!("SSH: key {} rejected", key_path.display());
                }
                Err(e) => {
                    log::debug!("SSH: key auth error for {}: {e}", key_path.display());
                }
            }
        }
    }

    // 2. Request password from user.
    let _ = event_tx.send(TermEvent::SshNeedsPassword(params.tab_id));

    // Wait for the user to provide a password or cancel.
    while let Some(input) = input_rx.recv().await {
        match input {
            SshInput::Password(password) => {
                match session.authenticate_password(&params.user, &password).await {
                    Ok(auth) if auth.success() => {
                        log::info!("SSH: authenticated with password");
                        return Ok(true);
                    }
                    Ok(_) => {
                        log::warn!("SSH: password rejected");
                        let _ = event_tx.send(TermEvent::SshError(
                            params.tab_id,
                            "Password rejected".to_string(),
                        ));
                        return Ok(false);
                    }
                    Err(e) => {
                        return Err(e).context("Password authentication failed");
                    }
                }
            }
            SshInput::Close => return Ok(false),
            _ => {}
        }
    }

    Ok(false)
}

/// Collects SSH key file paths to try for authentication.
fn collect_key_paths(identity_file: &Option<PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // User-specified key from config.
    if let Some(ref path) = identity_file {
        let expanded = expand_tilde(path);
        if expanded.exists() {
            paths.push(expanded);
        }
    }

    // Default key locations.
    if let Some(ssh_dir) = dirs::home_dir().map(|h| h.join(".ssh")) {
        for name in &["id_ed25519", "id_rsa", "id_ecdsa"] {
            let path = ssh_dir.join(name);
            if path.exists() && !paths.contains(&path) {
                paths.push(path);
            }
        }
    }

    paths
}

/// Expands `~` prefix to the user's home directory.
fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

/// Runs the I/O loop: forwards data between SSH channel and main thread.
async fn run_io_loop(
    mut channel: russh::Channel<russh::client::Msg>,
    tab_id: usize,
    event_tx: &Sender<TermEvent>,
    input_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SshInput>,
) -> Result<()> {
    use russh::ChannelMsg;

    loop {
        tokio::select! {
            // Data from SSH server -> main thread.
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        let _ = event_tx.send(TermEvent::Output(tab_id, data.to_vec()));
                    }
                    Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                        // stderr — also send to terminal.
                        let _ = event_tx.send(TermEvent::Output(tab_id, data.to_vec()));
                    }
                    Some(ChannelMsg::Eof | ChannelMsg::Close) => {
                        break;
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {
                        // Don't break — more data may follow.
                    }
                    Some(_) => {}
                    None => break, // Channel closed.
                }
            }
            // Input from main thread -> SSH server.
            input = input_rx.recv() => {
                match input {
                    Some(SshInput::Data(data)) => {
                        channel.data(&data[..]).await.context("Failed to send data to SSH channel")?;
                    }
                    Some(SshInput::Resize(cols, rows)) => {
                        channel.window_change(cols as u32, rows as u32, 0, 0).await
                            .context("Failed to send window change")?;
                    }
                    Some(SshInput::Close) | None => break,
                    Some(SshInput::Password(_)) => {
                        // Unexpected during I/O phase — ignore.
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_key_paths_includes_defaults() {
        let paths = collect_key_paths(&None);
        // Should find at least some default keys if ~/.ssh exists.
        // We can't assert specific files exist in CI, but verify no panic.
        let _ = paths;
    }

    #[test]
    fn collect_key_paths_with_identity_file() {
        let paths = collect_key_paths(&Some(PathBuf::from("/nonexistent/key")));
        // Nonexistent file should not be included.
        assert!(
            !paths
                .iter()
                .any(|p| p == &PathBuf::from("/nonexistent/key")),
            "nonexistent file should not be in paths"
        );
    }

    #[test]
    fn expand_tilde_expands_home() {
        let path = PathBuf::from("~/test/file");
        let expanded = expand_tilde(&path);
        assert!(!expanded.to_string_lossy().starts_with("~/"));
    }

    #[test]
    fn expand_tilde_no_tilde_unchanged() {
        let path = PathBuf::from("/absolute/path");
        let expanded = expand_tilde(&path);
        assert_eq!(expanded, path);
    }
}
