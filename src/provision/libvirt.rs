use std::path::PathBuf;

use crate::error::{DeployError, DeployResult};
use crate::provision::{Provisioner, ServerInfo};
use crate::ssh::SshSession;

/// Networking mode for the VM.
#[derive(Debug, Clone)]
pub enum NetworkMode {
    /// Bridge the VM onto an existing host bridge (e.g. `br0`).
    /// The VM gets an IP on the LAN, reachable from other hosts.
    Bridged(String),
    /// Use the default NAT network (`virbr0`).
    /// The VM can reach the internet but is only reachable from
    /// the hypervisor unless you add port forwards.
    Nat,
}

/// Libvirt/KVM provisioner for local or remote hypervisors.
///
/// Manages virtual machines via `virsh` and `virt-install` over
/// SSH. Cloud images are provisioned with cloud-init (`NoCloud`
/// datasource).
pub struct Libvirt {
    /// SSH hostname or IP of the hypervisor.
    pub hypervisor_host: String,
    /// SSH user on the hypervisor (default: `root`).
    pub hypervisor_user: String,
    /// Optional SSH private key for the hypervisor connection.
    pub hypervisor_key: Option<String>,
    /// Number of vCPUs (default: 2).
    pub vcpus: u32,
    /// RAM in MiB (default: 2048).
    pub memory_mib: u32,
    /// Disk size in GiB (default: 20).
    pub disk_gib: u32,
    /// Cloud image URL to download on the hypervisor.
    pub image_url: String,
    /// Network mode (default: NAT).
    pub network: NetworkMode,
    /// Directory on the hypervisor for VM disk images.
    pub storage_dir: String,
    /// Local SSH private key whose `.pub` sibling is injected
    /// via cloud-init. Used to SSH into the VM after creation.
    pub vm_ssh_key: String,
    /// `os-variant` passed to `virt-install`.
    pub os_variant: String,
}

impl Libvirt {
    /// Create a new Libvirt provisioner.
    ///
    /// # Arguments
    ///
    /// * `hypervisor_host` - SSH-reachable hostname of the KVM
    ///   host
    /// * `vm_ssh_key` - path to the local SSH private key; the
    ///   matching `.pub` file is read and injected into the VM
    ///   via cloud-init
    #[must_use]
    pub fn new(hypervisor_host: &str, vm_ssh_key: &str) -> Self {
        Self {
            hypervisor_host: hypervisor_host.to_string(),
            hypervisor_user: "root".to_string(),
            hypervisor_key: None,
            vcpus: 2,
            memory_mib: 2048,
            disk_gib: 20,
            image_url: "https://cloud-images.ubuntu.com/\
                releases/24.04/release/\
                ubuntu-24.04-server-cloudimg-amd64.img"
                .to_string(),
            network: NetworkMode::Nat,
            storage_dir: "/var/lib/libvirt/images".to_string(),
            vm_ssh_key: vm_ssh_key.to_string(),
            os_variant: "ubuntu24.04".to_string(),
        }
    }

    #[must_use]
    pub fn hypervisor_user(mut self, user: &str) -> Self {
        self.hypervisor_user = user.to_string();
        self
    }

    #[must_use]
    pub fn hypervisor_key(mut self, key: &str) -> Self {
        self.hypervisor_key = Some(key.to_string());
        self
    }

    #[must_use]
    pub const fn vcpus(mut self, n: u32) -> Self {
        self.vcpus = n;
        self
    }

    #[must_use]
    pub const fn memory_mib(mut self, mib: u32) -> Self {
        self.memory_mib = mib;
        self
    }

    #[must_use]
    pub const fn disk_gib(mut self, gib: u32) -> Self {
        self.disk_gib = gib;
        self
    }

    #[must_use]
    pub fn image_url(mut self, url: &str) -> Self {
        self.image_url = url.to_string();
        self
    }

    #[must_use]
    pub fn network(mut self, mode: NetworkMode) -> Self {
        self.network = mode;
        self
    }

    #[must_use]
    pub fn storage_dir(mut self, dir: &str) -> Self {
        self.storage_dir = dir.to_string();
        self
    }

    #[must_use]
    pub fn os_variant(mut self, variant: &str) -> Self {
        self.os_variant = variant.to_string();
        self
    }

    // -- private helpers --

    /// Open an SSH session to the hypervisor.
    fn hypervisor_ssh(&self) -> SshSession {
        let ssh = SshSession::new(&self.hypervisor_host, &self.hypervisor_user);
        if let Some(key) = &self.hypervisor_key {
            ssh.with_key(key)
        } else {
            ssh
        }
    }

    /// Read the public key content from `vm_ssh_key.pub`.
    fn read_pub_key(&self) -> DeployResult<String> {
        let pub_path = format!("{}.pub", self.vm_ssh_key);
        std::fs::read_to_string(&pub_path)
            .map_err(|_| DeployError::FileNotFound(format!("public key not found: {pub_path}")))
    }

    /// Create a `NoCloud` seed ISO on the hypervisor.
    ///
    /// Writes `user-data` and `meta-data` to a temp directory,
    /// then generates the ISO with genisoimage or mkisofs.
    fn create_seed_iso(&self, ssh: &SshSession, name: &str) -> DeployResult<String> {
        let pub_key = self.read_pub_key()?;
        let pub_key = pub_key.trim();

        let seed_dir = format!("/tmp/cloud-init-{name}");
        let iso_path = format!("{}/{name}-seed.iso", self.storage_dir);

        let user_data = format!(
            "#cloud-config\n\
             users:\n  \
               - name: root\n    \
                 ssh_authorized_keys:\n      \
                   - {pub_key}\n\
             ssh_pwauth: false\n\
             package_update: false\n"
        );

        let meta_data = format!("instance-id: {name}\nlocal-hostname: {name}\n");

        ssh.exec(&format!("mkdir -p {seed_dir}"))?;
        ssh.write_remote_file(&user_data, &format!("{seed_dir}/user-data"))?;
        ssh.write_remote_file(&meta_data, &format!("{seed_dir}/meta-data"))?;

        // Try genisoimage first, fall back to mkisofs
        let iso_cmd = format!(
            "if command -v genisoimage >/dev/null 2>&1; then \
               genisoimage -output {iso_path} -volid cidata \
               -joliet -rock {seed_dir}/user-data \
               {seed_dir}/meta-data; \
             else \
               mkisofs -output {iso_path} -volid cidata \
               -joliet -rock {seed_dir}/user-data \
               {seed_dir}/meta-data; \
             fi"
        );
        ssh.exec(&iso_cmd)?;
        ssh.exec(&format!("rm -rf {seed_dir}"))?;

        Ok(iso_path)
    }

    /// Poll `virsh domifaddr` until we get an IP.
    fn wait_for_ip(ssh: &SshSession, name: &str) -> DeployResult<String> {
        let max_attempts = 30;
        let interval = std::time::Duration::from_secs(5);

        for attempt in 1..=max_attempts {
            eprint!(
                "Waiting for IP \
                 ({attempt}/{max_attempts})... "
            );

            // Try the default agent/lease source first
            if let Ok(output) = ssh.exec(&format!("virsh domifaddr {name} 2>/dev/null")) {
                if let Some(ip) = parse_domifaddr(&output) {
                    eprintln!("got {ip}");
                    return Ok(ip);
                }
            }

            // Bridged networks often need --source arp
            if let Ok(output) = ssh.exec(&format!(
                "virsh domifaddr {name} \
                 --source arp 2>/dev/null"
            )) {
                if let Some(ip) = parse_domifaddr(&output) {
                    eprintln!("got {ip}");
                    return Ok(ip);
                }
            }

            eprintln!("not yet");
            std::thread::sleep(interval);
        }

        Err(DeployError::Other(format!(
            "VM '{name}' did not get an IP after \
             {max_attempts} attempts"
        )))
    }

    /// Run the remote setup script on the VM (not the
    /// hypervisor).
    fn run_setup_script(ssh: &SshSession, domain: &str, remote_dir: &str) -> DeployResult<()> {
        let script = include_str!("../../scripts/setup-server.sh");
        let escaped = script.replace('\'', "'\\''");
        ssh.exec_interactive(&format!("bash -c '{escaped}' _ '{domain}' '{remote_dir}'"))
    }

    /// Network arguments for virt-install.
    fn network_args(&self) -> String {
        match &self.network {
            NetworkMode::Bridged(bridge) => {
                format!("bridge={bridge}")
            }
            NetworkMode::Nat => "network=default".to_string(),
        }
    }
}

impl Provisioner for Libvirt {
    fn check_prerequisites(&self) -> DeployResult<()> {
        eprintln!("Checking prerequisites...");

        // Check local SSH key exists
        let key_path = PathBuf::from(&self.vm_ssh_key);
        if !key_path.exists() {
            return Err(DeployError::FileNotFound(format!(
                "VM SSH key not found: {}",
                self.vm_ssh_key
            )));
        }
        let pub_path = PathBuf::from(format!("{}.pub", self.vm_ssh_key));
        if !pub_path.exists() {
            return Err(DeployError::FileNotFound(format!(
                "VM SSH public key not found: {}.pub",
                self.vm_ssh_key
            )));
        }

        // Check hypervisor is reachable and has required tools
        let ssh = self.hypervisor_ssh();
        ssh.exec("echo ok").map_err(|_| {
            DeployError::PrerequisiteMissing(format!(
                "cannot SSH to hypervisor {}@{}",
                self.hypervisor_user, self.hypervisor_host
            ))
        })?;

        for tool in &["virsh", "virt-install", "qemu-img"] {
            ssh.exec(&format!("command -v {tool}")).map_err(|_| {
                DeployError::PrerequisiteMissing(format!("'{tool}' not found on hypervisor"))
            })?;
        }

        // Check for genisoimage or mkisofs
        let has_iso_tool = ssh
            .exec(
                "command -v genisoimage \
                 || command -v mkisofs",
            )
            .is_ok();
        if !has_iso_tool {
            return Err(DeployError::PrerequisiteMissing(
                "neither genisoimage nor mkisofs found on \
                 hypervisor (apt install genisoimage)"
                    .into(),
            ));
        }

        eprintln!("Prerequisites OK");
        Ok(())
    }

    fn detect_ssh_key(&self) -> DeployResult<(String, String)> {
        Ok((String::new(), self.vm_ssh_key.clone()))
    }

    fn create_server(
        &self,
        name: &str,
        _region: &str,
        _ssh_key_id: &str,
    ) -> DeployResult<ServerInfo> {
        let ssh = self.hypervisor_ssh();
        let disk_path = format!("{}/{name}.qcow2", self.storage_dir);

        eprintln!("Creating VM '{name}'...");

        // Download cloud image if not cached
        let cached = format!("{}/cloud-base.img", self.storage_dir);
        let has_cache = ssh
            .exec(&format!("test -f {cached} && echo yes"))
            .unwrap_or_default();
        if has_cache.trim() != "yes" {
            eprintln!("Downloading cloud image...");
            ssh.exec(&format!("wget -q -O {cached} '{}'", self.image_url))?;
        }

        // Create disk from base image and resize
        ssh.exec(&format!("cp {cached} {disk_path}"))?;
        ssh.exec(&format!("qemu-img resize {disk_path} {}G", self.disk_gib))?;

        // Create cloud-init seed ISO
        let seed_iso = self.create_seed_iso(&ssh, name)?;

        // Run virt-install
        let net_arg = self.network_args();
        let install_cmd = format!(
            "virt-install \
             --name {name} \
             --vcpus {} \
             --memory {} \
             --disk path={disk_path},format=qcow2 \
             --disk path={seed_iso},device=cdrom \
             --os-variant {} \
             --network {net_arg} \
             --graphics none \
             --noautoconsole \
             --import",
            self.vcpus, self.memory_mib, self.os_variant
        );
        ssh.exec(&install_cmd)?;

        // Wait for VM to get an IP
        let ip = Self::wait_for_ip(&ssh, name)?;
        eprintln!("VM created! IP: {ip}");

        Ok(ServerInfo {
            name: name.to_string(),
            ip,
            region: "local".to_string(),
            ssh_key_id: String::new(),
            ssh_key_file: self.vm_ssh_key.clone(),
        })
    }

    fn setup_server(&self, server: &ServerInfo, domain: Option<&str>) -> DeployResult<()> {
        // SSH to the VM itself, not the hypervisor
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
        eprintln!("VM provisioned successfully!");
        eprintln!("========================================");
        eprintln!();
        eprintln!("VM: {}", server.name);
        eprintln!("IP: {}", server.ip);
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
        let ssh = self.hypervisor_ssh();

        // Check if domain exists
        let Ok(state) = ssh.exec(&format!("virsh domstate {name} 2>/dev/null")) else {
            return Ok(None);
        };

        if state.trim().is_empty() {
            return Ok(None);
        }

        // Quick IP lookup (3 attempts)
        for _ in 0..3 {
            if let Ok(output) = ssh.exec(&format!("virsh domifaddr {name} 2>/dev/null")) {
                if let Some(ip) = parse_domifaddr(&output) {
                    return Ok(Some(ServerInfo {
                        name: name.to_string(),
                        ip,
                        region: "local".to_string(),
                        ssh_key_id: String::new(),
                        ssh_key_file: self.vm_ssh_key.clone(),
                    }));
                }
            }
            if let Ok(output) = ssh.exec(&format!(
                "virsh domifaddr {name} \
                 --source arp 2>/dev/null"
            )) {
                if let Some(ip) = parse_domifaddr(&output) {
                    return Ok(Some(ServerInfo {
                        name: name.to_string(),
                        ip,
                        region: "local".to_string(),
                        ssh_key_id: String::new(),
                        ssh_key_file: self.vm_ssh_key.clone(),
                    }));
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // Domain exists but no IP yet
        Ok(Some(ServerInfo {
            name: name.to_string(),
            ip: String::new(),
            region: "local".to_string(),
            ssh_key_id: String::new(),
            ssh_key_file: self.vm_ssh_key.clone(),
        }))
    }

    fn destroy_server(&self, name: &str) -> DeployResult<()> {
        let ssh = self.hypervisor_ssh();

        eprintln!("Destroying VM '{name}'...");

        // Force stop if running
        let _ = ssh.exec(&format!("virsh destroy {name} 2>/dev/null"));

        // Undefine and remove storage
        ssh.exec(&format!(
            "virsh undefine {name} \
             --remove-all-storage 2>/dev/null || true"
        ))?;

        // Remove seed ISO if it exists
        let seed_iso = format!("{}/{name}-seed.iso", self.storage_dir);
        let _ = ssh.exec(&format!("rm -f {seed_iso}"));

        eprintln!("VM '{name}' destroyed");

        // Remove SSH config entry
        super::remove_ssh_config_entry(name)?;

        Ok(())
    }
}

/// Parse an IP address from `virsh domifaddr` output.
///
/// Handles both the default (agent/lease) format and the
/// `--source arp` format.
///
/// # Examples
///
/// Default output:
/// ```text
///  Name       MAC address          Protocol     Address
/// -------------------------------------------------------
///  vnet0      52:54:00:ab:cd:ef    ipv4         192.168.122.45/24
/// ```
///
/// ARP output:
/// ```text
///  Name       MAC address          Protocol     Address
/// -------------------------------------------------------
///  vnet0      52:54:00:ab:cd:ef    ipv4         10.0.0.50
/// ```
#[must_use]
pub fn parse_domifaddr(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        // Skip header and separator lines
        if trimmed.is_empty() || trimmed.starts_with("Name") || trimmed.starts_with('-') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        // Expect: name, mac, protocol, address
        if parts.len() >= 4 && parts[2] == "ipv4" {
            let addr = parts[3];
            // Strip CIDR suffix if present (e.g. /24)
            let ip = addr.split('/').next().unwrap_or(addr);
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }
    None
}
