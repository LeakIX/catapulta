pub mod digitalocean;
pub mod libvirt;

use std::path::PathBuf;

use crate::error::{DeployError, DeployResult};

/// Information about a provisioned server.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub ip: String,
    pub region: String,
    pub ssh_key_id: String,
    pub ssh_key_file: String,
}

/// A provisioner creates, configures, and destroys cloud servers.
pub trait Provisioner {
    /// Check that all prerequisites are installed and
    /// authenticated.
    fn check_prerequisites(&self) -> DeployResult<()>;

    /// Detect the SSH key to use for provisioning.
    ///
    /// Returns `(key_id, key_file)` where `key_id` is the
    /// provider-specific identifier and `key_file` is the local
    /// private key path.
    fn detect_ssh_key(&self) -> DeployResult<(String, String)> {
        Ok((String::new(), String::new()))
    }

    /// Create a new server and return its info.
    fn create_server(&self, name: &str, region: &str, ssh_key_id: &str)
    -> DeployResult<ServerInfo>;

    /// Install Docker, configure firewall, start Caddy
    /// placeholder.
    fn setup_server(&self, server: &ServerInfo, domain: Option<&str>) -> DeployResult<()>;

    /// Get an existing server by name.
    fn get_server(&self, name: &str) -> DeployResult<Option<ServerInfo>>;

    /// Destroy a server by name.
    fn destroy_server(&self, name: &str) -> DeployResult<()>;
}

/// Remove a Host block from SSH config content.
#[must_use]
pub fn remove_ssh_host_entry(content: &str, host: &str) -> String {
    let mut result = Vec::new();
    let mut skip = false;
    let header = format!("Host {host}");

    for line in content.lines() {
        if line.trim() == header {
            skip = true;
            continue;
        }
        if skip {
            // If we hit a new Host block or a non-indented line
            // (that isn't empty), stop skipping
            if !line.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                skip = false;
                result.push(line);
            }
            continue;
        }
        result.push(line);
    }

    let mut out = result.join("\n");
    // Clean up multiple blank lines
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out
}

/// Add an entry to `~/.ssh/config` for a server.
pub fn setup_ssh_config(ip: &str, host_alias: &str, key_file: &str) -> DeployResult<()> {
    let home = std::env::var("HOME").map_err(|_| DeployError::EnvMissing("HOME".into()))?;
    let config_path = PathBuf::from(&home).join(".ssh").join("config");

    let mut content = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    // Remove existing entry for this host alias
    content = remove_ssh_host_entry(&content, host_alias);

    // Append new entry
    let entry = format!(
        "\nHost {host_alias}\n    \
         HostName {ip}\n    \
         User root\n    \
         IdentityFile {key_file}\n    \
         StrictHostKeyChecking no\n"
    );
    content.push_str(&entry);

    std::fs::write(&config_path, &content)?;
    eprintln!("SSH config: ssh {host_alias}");
    Ok(())
}

/// Remove an SSH host entry from `~/.ssh/config`.
pub fn remove_ssh_config_entry(host_alias: &str) -> DeployResult<()> {
    let home = std::env::var("HOME").map_err(|_| DeployError::EnvMissing("HOME".into()))?;
    let config_path = PathBuf::from(&home).join(".ssh").join("config");

    if !config_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&config_path)?;
    let updated = remove_ssh_host_entry(&content, host_alias);
    std::fs::write(&config_path, updated)?;

    eprintln!("SSH config entry removed: {host_alias}");
    Ok(())
}
