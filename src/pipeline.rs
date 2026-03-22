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

/// Action to run on the remote host after deployment.
enum PostDeployHook {
    /// Upload a local file to a remote path.
    Upload { local: String, remote: String },
    /// Copy a local file into a running container.
    DockerCp {
        local: String,
        container: String,
        path: String,
    },
    /// Execute a shell command on the remote host.
    Exec(String),
}

/// Deployment pipeline orchestrating provisioning, DNS, and
/// deployment.
pub struct Pipeline {
    apps: Vec<App>,
    caddy: Caddy,
    provisioner: Option<Box<dyn Provisioner>>,
    dns: Vec<Box<dyn DnsProvider>>,
    deployer: Option<Box<dyn Deployer>>,
    remote_dir: String,
    ssh_user: String,
    post_deploy: Vec<PostDeployHook>,
}

impl Pipeline {
    /// Create a pipeline for a single app.
    #[must_use]
    pub fn new(app: App, caddy: Caddy) -> Self {
        Self {
            apps: vec![app],
            caddy,
            provisioner: None,
            dns: Vec::new(),
            deployer: None,
            remote_dir: "/opt/app".to_string(),
            ssh_user: "root".to_string(),
            post_deploy: Vec::new(),
        }
    }

    /// Create a pipeline for multiple apps behind one Caddy
    /// reverse proxy.
    #[must_use]
    pub fn multi(apps: Vec<App>, caddy: Caddy) -> Self {
        Self {
            apps,
            caddy,
            provisioner: None,
            dns: Vec::new(),
            deployer: None,
            remote_dir: "/opt/app".to_string(),
            ssh_user: "root".to_string(),
            post_deploy: Vec::new(),
        }
    }

    #[must_use]
    pub fn provision(mut self, provisioner: impl Provisioner + 'static) -> Self {
        self.provisioner = Some(Box::new(provisioner));
        self
    }

    #[must_use]
    pub fn dns(mut self, provider: impl DnsProvider + 'static) -> Self {
        self.dns.push(Box::new(provider));
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

    /// Upload a local file to the remote host after deployment.
    ///
    /// The remote path can be absolute or relative to the remote
    /// deployment directory. Skipped during `--dry-run`.
    #[must_use]
    pub fn upload(mut self, local: &str, remote: &str) -> Self {
        self.post_deploy.push(PostDeployHook::Upload {
            local: local.to_string(),
            remote: remote.to_string(),
        });
        self
    }

    /// Copy a local file into a running container after
    /// deployment.
    ///
    /// Uploads the file to the remote host via SCP, then runs
    /// `docker cp` to place it inside the container. The
    /// temporary remote copy is removed afterwards.
    #[must_use]
    pub fn docker_cp(mut self, local: &str, container: &str, path: &str) -> Self {
        self.post_deploy.push(PostDeployHook::DockerCp {
            local: local.to_string(),
            container: container.to_string(),
            path: path.to_string(),
        });
        self
    }

    /// Execute a shell command on the remote host after
    /// deployment.
    ///
    /// Commands run in order after containers are healthy.
    /// Skipped during `--dry-run`.
    #[must_use]
    pub fn after_deploy(mut self, command: &str) -> Self {
        self.post_deploy
            .push(PostDeployHook::Exec(command.to_string()));
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
            Command::Destroy {
                name,
                domain,
                force,
            } => self.cmd_destroy(name, domain.as_deref(), *force),
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
        let (key_id, _) = provisioner.detect_ssh_key()?;

        let region = region.unwrap_or("fra1");

        // Setup DNS before server setup so the domain resolves
        // by the time Caddy requests a TLS certificate
        let server = provisioner.create_server(name, region, &key_id)?;

        if domain.is_some() {
            for dns in &self.dns {
                let d = dns.domain();
                eprintln!("Setting up DNS for {d}...");
                dns.upsert_a_record(&server.ip)?;
                eprintln!("DNS record set: {d} -> {}", server.ip);
            }
        }

        provisioner.setup_server(&server, domain)?;

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
            for app in &self.apps {
                deployer.build_image(app)?;
            }
        }

        // Stop containers before loading to free memory on
        // constrained VPS instances.
        // When a maintenance page is configured, keep Caddy
        // running so it can serve the maintenance page while
        // app containers are down.
        eprintln!("Stopping containers...");
        let ssh = SshSession::new(host, &self.ssh_user);
        if self.caddy.maintenance_page.is_some() {
            // First, deploy updated Caddyfile with handle_errors
            // so Caddy can serve the maintenance page.
            let caddyfile_content =
                caddyfile::render(&self.caddy, host);
            ssh.write_remote_file(
                &caddyfile_content,
                &format!("{}/Caddyfile", self.remote_dir),
            )?;
            // Reload Caddy config if it's running
            ssh.exec(&format!(
                "cd {} && docker compose exec -T caddy \
                 caddy reload --config /etc/caddy/Caddyfile \
                 2>/dev/null || true",
                self.remote_dir,
            ))?;
            // Only stop app containers, keep Caddy running
            let app_names: Vec<&str> =
                self.apps.iter().map(|a| a.name.as_str()).collect();
            let names = app_names.join(" ");
            ssh.exec(&format!(
                "cd {} && docker compose rm -sf {} \
                 2>/dev/null || true",
                self.remote_dir, names,
            ))?;
        } else {
            ssh.exec(&format!(
                "cd {} && docker compose down \
                 2>/dev/null || true",
                self.remote_dir
            ))?;
        }

        for app in &self.apps {
            deployer.transfer_image(app, host, &self.ssh_user)?;
        }

        deployer.deploy(
            host,
            &self.ssh_user,
            &self.apps,
            &self.caddy,
            &self.remote_dir,
        )?;

        if !self.post_deploy.is_empty() {
            eprintln!("Running post-deploy hooks...");
            let ssh = SshSession::new(host, &self.ssh_user);
            for hook in &self.post_deploy {
                match hook {
                    PostDeployHook::Upload { local, remote } => {
                        eprintln!("  Uploading {local} -> {remote}");
                        ssh.scp_to(local, remote)?;
                    }
                    PostDeployHook::DockerCp {
                        local,
                        container,
                        path,
                    } => {
                        let filename = std::path::Path::new(local)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy();
                        let tmp = format!("/tmp/catapulta-cp-{filename}");
                        eprintln!(
                            "  docker cp {local} -> \
                             {container}:{path}"
                        );
                        ssh.scp_to(local, &tmp)?;
                        ssh.exec_interactive(&format!(
                            "docker cp {tmp} {container}:{path} \
                             && rm -f {tmp}"
                        ))?;
                    }
                    PostDeployHook::Exec(cmd) => {
                        eprintln!("  Running: {cmd}");
                        ssh.exec_interactive(cmd)?;
                    }
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    fn cmd_deploy_dry_run(&self, host: &str) -> DeployResult<()> {
        let compose_content = compose::render(&self.apps, &self.caddy);
        let caddyfile_content = caddyfile::render(&self.caddy, host);

        eprintln!("=== Dry run: no changes will be made ===");
        eprintln!();

        eprintln!("--- docker-compose.yml ---");
        println!("{compose_content}");

        eprintln!("--- Caddyfile ---");
        println!("{caddyfile_content}");

        eprintln!("--- Actions that would be performed ---");
        for (i, app) in self.apps.iter().enumerate() {
            let n = i + 1;
            eprintln!("{n}. Build Docker image: {}:latest", app.name);
        }
        let base = self.apps.len();
        for (i, app) in self.apps.iter().enumerate() {
            let n = base + i + 1;
            eprintln!("{n}. Transfer {} to {}@{}", app.name, self.ssh_user, host);
        }
        let mut step = base * 2 + 1;
        eprintln!("{step}. Write config files to {}/", self.remote_dir);
        step += 1;
        let has_env = self.apps.iter().any(|a| a.env_file.is_some());
        if has_env {
            eprintln!("{step}. Transfer .env file(s)");
            step += 1;
        }
        eprintln!("{step}. Restart containers via docker compose");

        if !self.post_deploy.is_empty() {
            eprintln!();
            eprintln!("--- Post-deploy hooks ---");
            for (i, hook) in self.post_deploy.iter().enumerate() {
                let n = i + 1;
                match hook {
                    PostDeployHook::Upload { local, remote } => {
                        eprintln!("{n}. Upload {local} -> {remote}");
                    }
                    PostDeployHook::DockerCp {
                        local,
                        container,
                        path,
                    } => {
                        eprintln!(
                            "{n}. docker cp {local} -> \
                             {container}:{path}"
                        );
                    }
                    PostDeployHook::Exec(cmd) => {
                        eprintln!("{n}. Run: {cmd}");
                    }
                }
            }
        }

        Ok(())
    }

    fn cmd_status(&self, host: &str) -> DeployResult<()> {
        let ssh = SshSession::new(host, &self.ssh_user);
        ssh.exec_interactive(&format!("cd {} && docker compose ps", self.remote_dir))
    }

    fn cmd_destroy(&self, name: &str, domain: Option<&str>, force: bool) -> DeployResult<()> {
        let provisioner = self
            .provisioner
            .as_ref()
            .ok_or_else(|| DeployError::Other("no provisioner configured".into()))?;

        // Show what will be destroyed
        eprintln!(
            "WARNING: This will permanently delete \
             droplet '{name}'"
        );
        if domain.is_some() {
            for dns in &self.dns {
                eprintln!("and DNS record for {}", dns.domain());
            }
        }
        eprintln!();

        if !force {
            // Ask for confirmation
            eprint!("Are you sure? Type 'yes' to confirm: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim() != "yes" {
                eprintln!("Aborted.");
                return Ok(());
            }
        }

        provisioner.destroy_server(name)?;

        // Remove DNS records
        if domain.is_some() {
            for dns in &self.dns {
                let d = dns.domain();
                eprintln!("Removing DNS record for {d}...");
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

        /// Skip interactive confirmation prompt
        #[arg(long)]
        force: bool,
    },
}
