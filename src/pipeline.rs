use clap::{Parser, Subcommand};

use crate::app::App;
use crate::caddy::Caddy;
use crate::caddyfile;
use crate::compose;
use crate::deploy::Deployer;
use crate::dns::DnsProvider;
use crate::error::{DeployError, DeployResult};
use crate::provision::Provisioner;
use crate::ssh::SshSession;

/// Deployment pipeline orchestrating provisioning, DNS, and
/// deployment.
pub struct Pipeline {
    app: App,
    caddy: Caddy,
    provisioner: Option<Box<dyn Provisioner>>,
    dns: Option<Box<dyn DnsProvider>>,
    deployer: Option<Box<dyn Deployer>>,
    remote_dir: String,
    ssh_user: String,
}

impl Pipeline {
    #[must_use]
    pub fn new(app: App, caddy: Caddy) -> Self {
        Self {
            app,
            caddy,
            provisioner: None,
            dns: None,
            deployer: None,
            remote_dir: "/opt/app".to_string(),
            ssh_user: "root".to_string(),
        }
    }

    #[must_use]
    pub fn provision(mut self, provisioner: impl Provisioner + 'static) -> Self {
        self.provisioner = Some(Box::new(provisioner));
        self
    }

    #[must_use]
    pub fn dns(mut self, provider: impl DnsProvider + 'static) -> Self {
        self.dns = Some(Box::new(provider));
        self
    }

    #[must_use]
    pub fn deploy(mut self, deployer: impl Deployer + 'static) -> Self {
        self.deployer = Some(Box::new(deployer));
        self
    }

    #[must_use]
    pub fn remote_dir(mut self, dir: &str) -> Self {
        self.remote_dir = dir.to_string();
        self
    }

    #[must_use]
    pub fn ssh_user(mut self, user: &str) -> Self {
        self.ssh_user = user.to_string();
        self
    }

    /// Parse CLI arguments and dispatch the appropriate
    /// command.
    ///
    /// # Errors
    ///
    /// Returns an error if the dispatched command fails.
    pub fn run(&self) -> DeployResult<()> {
        let cli = Cli::parse();

        match &cli.command {
            Command::Provision {
                name,
                domain,
                region,
            } => self.cmd_provision(name, domain.as_deref(), region.as_deref()),
            Command::Deploy {
                host,
                skip_build,
                dry_run,
            } => self.cmd_deploy(host, *skip_build, *dry_run),
            Command::Status { host } => self.cmd_status(host),
            Command::Destroy { name, domain } => self.cmd_destroy(name, domain.as_deref()),
        }
    }

    fn cmd_provision(
        &self,
        name: &str,
        domain: Option<&str>,
        region: Option<&str>,
    ) -> DeployResult<()> {
        let provisioner = self
            .provisioner
            .as_ref()
            .ok_or_else(|| DeployError::Other("no provisioner configured".into()))?;

        provisioner.check_prerequisites()?;

        // Check if already exists
        if let Some(existing) = provisioner.get_server(name)? {
            eprintln!(
                "Droplet '{name}' already exists \
                 (IP: {})",
                existing.ip
            );
            let host = domain.unwrap_or(&existing.ip);
            eprintln!("Deploy with:");
            eprintln!("  cargo xtask deploy {host}");
            return Ok(());
        }

        // Detect SSH key
        let (key_id, _) = detect_do_ssh_key()?;

        let region = region.unwrap_or("fra1");

        // Setup DNS before server setup so the domain resolves
        // by the time Caddy requests a TLS certificate
        let server = provisioner.create_server(name, region, &key_id)?;

        if let (Some(dns), Some(d)) = (&self.dns, domain) {
            eprintln!("Setting up DNS...");
            dns.upsert_a_record(&server.ip)?;
            eprintln!("DNS record set: {d} -> {}", server.ip);
        }

        provisioner.setup_server(&server, &self.app, &self.caddy, domain)?;

        Ok(())
    }

    fn cmd_deploy(&self, host: &str, skip_build: bool, dry_run: bool) -> DeployResult<()> {
        if dry_run {
            return self.cmd_deploy_dry_run(host);
        }

        let deployer = self
            .deployer
            .as_ref()
            .ok_or_else(|| DeployError::Other("no deployer configured".into()))?;

        if !skip_build {
            deployer.build_image(&self.app)?;
        }

        deployer.transfer_image(&self.app, host, &self.ssh_user)?;

        deployer.deploy(
            host,
            &self.ssh_user,
            &self.app,
            &self.caddy,
            &self.remote_dir,
        )?;

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn cmd_deploy_dry_run(&self, host: &str) -> DeployResult<()> {
        let compose_content = compose::render(&self.app, &self.caddy);
        let caddyfile_content = caddyfile::render(&self.caddy, host);

        eprintln!("=== Dry run: no changes will be made ===");
        eprintln!();

        eprintln!("--- docker-compose.yml ---");
        println!("{compose_content}");

        eprintln!("--- Caddyfile ---");
        println!("{caddyfile_content}");

        eprintln!("--- Actions that would be performed ---");
        eprintln!("1. Build Docker image: {}:latest", self.app.name);
        eprintln!("2. Transfer image to {}@{}", self.ssh_user, host);
        eprintln!("3. Write config files to {}/", self.remote_dir);
        if self.app.env_file.is_some() {
            eprintln!("4. Transfer .env file");
            eprintln!("5. Restart containers via docker compose");
        } else {
            eprintln!("4. Restart containers via docker compose");
        }

        Ok(())
    }

    fn cmd_status(&self, host: &str) -> DeployResult<()> {
        let ssh = SshSession::new(host, &self.ssh_user);
        ssh.exec_interactive(&format!("cd {} && docker compose ps", self.remote_dir))
    }

    fn cmd_destroy(&self, name: &str, domain: Option<&str>) -> DeployResult<()> {
        let provisioner = self
            .provisioner
            .as_ref()
            .ok_or_else(|| DeployError::Other("no provisioner configured".into()))?;

        // Show what will be destroyed
        eprintln!(
            "WARNING: This will permanently delete \
             droplet '{name}'"
        );
        if let Some(d) = domain {
            eprintln!("and DNS record for {d}");
        }
        eprintln!();

        // Ask for confirmation
        eprint!("Are you sure? Type 'yes' to confirm: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            eprintln!("Aborted.");
            return Ok(());
        }

        provisioner.destroy_server(name)?;

        // Remove DNS record
        if let Some(dns) = &self.dns {
            if domain.is_some() {
                eprintln!("Removing DNS record...");
                dns.delete_a_record()?;
            }
        }

        eprintln!();
        eprintln!("Cleanup complete!");

        Ok(())
    }
}

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Deployment automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Provision a new server
    Provision {
        /// Server name
        name: String,

        /// Domain to point at the server
        #[arg(long)]
        domain: Option<String>,

        /// Cloud region
        #[arg(long)]
        region: Option<String>,
    },

    /// Deploy to a server
    Deploy {
        /// Hostname or IP address
        host: String,

        /// Skip Docker image build
        #[arg(long)]
        skip_build: bool,

        /// Preview generated files without executing
        #[arg(long)]
        dry_run: bool,
    },

    /// Show container status on a remote server
    Status {
        /// Hostname or IP address
        host: String,
    },

    /// Destroy a server
    Destroy {
        /// Server name
        name: String,

        /// Domain record to remove
        #[arg(long)]
        domain: Option<String>,
    },
}

/// Detect SSH key registered with `DigitalOcean`. Returns
/// (`key_id`, `key_file`).
fn detect_do_ssh_key() -> DeployResult<(String, String)> {
    use crate::cmd;

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

    Ok((parts[0].to_string(), parts[1].to_string()))
}
