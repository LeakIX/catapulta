use std::path::PathBuf;

use crate::cmd;
use crate::error::{DeployError, DeployResult};
use crate::provision::{Provisioner, ServerInfo};
use crate::ssh::SshSession;

/// `DigitalOcean` provisioner using `doctl` CLI.
pub struct DigitalOcean {
    pub size: String,
    pub region: String,
    pub image: String,
}

impl DigitalOcean {
    #[must_use]
    pub fn new() -> Self {
        Self {
            size: "s-1vcpu-1gb".to_string(),
            region: "fra1".to_string(),
            image: "ubuntu-24-04-x64".to_string(),
        }
    }

    #[must_use]
    pub fn size(mut self, size: &str) -> Self {
        self.size = size.to_string();
        self
    }

    #[must_use]
    pub fn region(mut self, region: &str) -> Self {
        self.region = region.to_string();
        self
    }

    #[must_use]
    pub fn image(mut self, image: &str) -> Self {
        self.image = image.to_string();
        self
    }

    /// Detect the SSH key registered with `DigitalOcean` and
    /// find the matching local private key.
    fn detect_do_ssh_key() -> DeployResult<(String, String)> {
        let output = cmd::run(
            "doctl",
            &[
                "compute",
                "ssh-key",
                "list",
                "--format",
                "ID,FingerPrint",
                "--no-header",
            ],
        )?;

        let first_line = output.lines().next().ok_or_else(|| {
            DeployError::PrerequisiteMissing("no SSH keys found in DigitalOcean".into())
        })?;

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(DeployError::PrerequisiteMissing(
                "unexpected doctl ssh-key list format".into(),
            ));
        }

        let key_id = parts[0].to_string();
        let do_fingerprint = parts[1];

        // Find matching local key
        let home = std::env::var("HOME").map_err(|_| DeployError::EnvMissing("HOME".into()))?;
        let ssh_dir = PathBuf::from(&home).join(".ssh");

        let pub_keys: Vec<PathBuf> = std::fs::read_dir(&ssh_dir)
            .map_err(|_| DeployError::FileNotFound("~/.ssh directory not found".into()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "pub"))
            .collect();

        for pub_key in &pub_keys {
            let pub_key_str = pub_key.to_string_lossy().to_string();
            let local_fp = cmd::run("ssh-keygen", &["-l", "-E", "md5", "-f", &pub_key_str]);

            if let Ok(fp_output) = local_fp {
                let local_fingerprint = fp_output
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .strip_prefix("MD5:")
                    .unwrap_or("");

                if local_fingerprint == do_fingerprint {
                    // Private key is the pub key path without
                    // .pub extension
                    let private_key = pub_key_str
                        .strip_suffix(".pub")
                        .unwrap_or(&pub_key_str)
                        .to_string();
                    eprintln!(
                        "SSH key: {private_key} \
                         (ID: {key_id})"
                    );
                    return Ok((key_id, private_key));
                }
            }
        }

        Err(DeployError::PrerequisiteMissing(format!(
            "no local key matches DO fingerprint \
             {do_fingerprint}"
        )))
    }

    fn get_droplet_ip(name: &str) -> DeployResult<String> {
        let output = cmd::run(
            "doctl",
            &[
                "compute",
                "droplet",
                "list",
                "--format",
                "Name,PublicIPv4",
                "--no-header",
            ],
        )?;

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == name {
                return Ok(parts[1].to_string());
            }
        }

        Err(DeployError::ServerNotFound(name.into()))
    }

    /// Run the remote setup script over SSH.
    fn run_setup_script(ssh: &SshSession, domain: &str, remote_dir: &str) -> DeployResult<()> {
        let script = include_str!("../../scripts/setup-server.sh");
        let escaped = script.replace('\'', "'\\''");
        ssh.exec_interactive(&format!("bash -c '{escaped}' _ '{domain}' '{remote_dir}'"))
    }
}

impl Default for DigitalOcean {
    fn default() -> Self {
        Self::new()
    }
}

impl Provisioner for DigitalOcean {
    fn check_prerequisites(&self) -> DeployResult<()> {
        eprintln!("Checking prerequisites...");

        if !cmd::command_exists("doctl") {
            return Err(DeployError::PrerequisiteMissing(
                "doctl is not installed. \
                 Install with: brew install doctl"
                    .into(),
            ));
        }

        cmd::run("doctl", &["account", "get"]).map_err(|_| {
            DeployError::PrerequisiteMissing(
                "doctl is not authenticated. \
                 Run: doctl auth init"
                    .into(),
            )
        })?;

        eprintln!("Prerequisites OK");
        Ok(())
    }

    fn detect_ssh_key(&self) -> DeployResult<(String, String)> {
        Self::detect_do_ssh_key()
    }

    fn create_server(
        &self,
        name: &str,
        region: &str,
        ssh_key_id: &str,
    ) -> DeployResult<ServerInfo> {
        eprintln!("Creating droplet '{name}' in {region}...");

        cmd::run_interactive(
            "doctl",
            &[
                "compute",
                "droplet",
                "create",
                name,
                "--image",
                &self.image,
                "--size",
                &self.size,
                "--region",
                region,
                "--ssh-keys",
                ssh_key_id,
                "--enable-monitoring",
                "--wait",
            ],
        )?;

        let ip = Self::get_droplet_ip(name)?;
        eprintln!("Droplet created! IP: {ip}");

        // We need to find the SSH key file again for the
        // ServerInfo - detect_ssh_key provides both id and
        // file.
        let (_, key_file) = Self::detect_do_ssh_key()?;

        Ok(ServerInfo {
            name: name.to_string(),
            ip,
            region: region.to_string(),
            ssh_key_id: ssh_key_id.to_string(),
            ssh_key_file: key_file,
        })
    }

    fn setup_server(&self, server: &ServerInfo, domain: Option<&str>) -> DeployResult<()> {
        let ssh = SshSession::new(&server.ip, "root").with_key(&server.ssh_key_file);

        ssh.wait_for_ready(30, std::time::Duration::from_secs(10))?;

        let domain_str = domain.unwrap_or(&server.ip);
        let remote_dir = "/opt/app";

        Self::run_setup_script(&ssh, domain_str, remote_dir)?;

        // Setup SSH config
        let host_alias = domain.unwrap_or(&server.name);
        super::setup_ssh_config(&server.ip, host_alias, &server.ssh_key_file)?;

        eprintln!();
        eprintln!("========================================");
        eprintln!("Droplet provisioned successfully!");
        eprintln!("========================================");
        eprintln!();
        eprintln!("Droplet: {}", server.name);
        eprintln!("IP: {}", server.ip);
        eprintln!("Region: {}", server.region);
        if let Some(d) = domain {
            eprintln!("Domain: {d}");
        }
        let deploy_host = domain.unwrap_or(&server.ip);
        eprintln!("SSH: ssh {deploy_host}");
        eprintln!();
        eprintln!("Deploy with:");
        eprintln!("  cargo xtask deploy {deploy_host}");
        eprintln!();

        Ok(())
    }

    fn get_server(&self, name: &str) -> DeployResult<Option<ServerInfo>> {
        let output = cmd::run(
            "doctl",
            &[
                "compute",
                "droplet",
                "list",
                "--format",
                "Name,PublicIPv4,Region",
                "--no-header",
            ],
        )?;

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[0] == name {
                let (_, key_file) = Self::detect_do_ssh_key()?;
                return Ok(Some(ServerInfo {
                    name: name.to_string(),
                    ip: parts[1].to_string(),
                    region: parts[2].to_string(),
                    ssh_key_id: String::new(),
                    ssh_key_file: key_file,
                }));
            }
        }

        Ok(None)
    }

    fn destroy_server(&self, name: &str) -> DeployResult<()> {
        let output = cmd::run(
            "doctl",
            &[
                "compute",
                "droplet",
                "list",
                "--format",
                "Name,ID",
                "--no-header",
            ],
        )?;

        let droplet_id = output
            .lines()
            .find_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == name {
                    Some(parts[1].to_string())
                } else {
                    None
                }
            })
            .ok_or_else(|| DeployError::ServerNotFound(name.into()))?;

        eprintln!("Deleting droplet '{name}'...");
        cmd::run(
            "doctl",
            &["compute", "droplet", "delete", &droplet_id, "--force"],
        )?;
        eprintln!("Droplet '{name}' deleted");

        // Remove SSH config entry
        super::remove_ssh_config_entry(name)?;

        Ok(())
    }
}
