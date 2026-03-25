use clap::{Parser, Subcommand};

use crate::app::App;
use crate::caddy::Caddy;
use crate::caddyfile;
use crate::cmd;
use crate::compose;
use crate::deploy::Deployer;
use crate::deploy::local::LocalDeploy;
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
    local_dir: String,
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
            local_dir: ".catapulta".to_string(),
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
            local_dir: ".catapulta".to_string(),
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

    #[must_use]
    pub fn local_dir(mut self, dir: &str) -> Self {
        self.local_dir = dir.to_string();
        self
    }

    /// Validate that all `--only` names match configured apps.
    fn validate_only(&self, only: &[String]) -> DeployResult<()> {
        for name in only {
            if !self.apps.iter().any(|a| a.name == *name) {
                let known: Vec<&str> = self.apps.iter().map(|a| a.name.as_str()).collect();
                return Err(DeployError::Other(format!(
                    "unknown service '{}'. \
                     Known services: {}",
                    name,
                    known.join(", ")
                )));
            }
        }
        Ok(())
    }

    /// Return apps filtered by `--only`, or all apps when empty.
    fn selected_apps(&self, only: &[String]) -> Vec<&App> {
        if only.is_empty() {
            self.apps.iter().collect()
        } else {
            self.apps
                .iter()
                .filter(|a| only.contains(&a.name))
                .collect()
        }
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
                only,
            } => self.cmd_deploy(host, *skip_build, *dry_run, only),
            Command::DeployLocal {
                domain,
                skip_build,
                dry_run,
                only,
            } => self.cmd_deploy_local(domain, *skip_build, *dry_run, only),
            Command::LocalDown => self.cmd_local_down(),
            Command::LocalStatus => self.cmd_local_status(),
            Command::Status { host } => self.cmd_status(host),
            Command::Destroy { name, force } => self.cmd_destroy(name, *force),
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

            // Update DNS to point at the current IP
            if domain.is_some() {
                for dns in &self.dns {
                    let d = dns.domain();
                    eprintln!("Updating DNS for {d}...");
                    dns.upsert_a_record(&existing.ip)?;
                    eprintln!("DNS record set: {d} -> {}", existing.ip);
                }
            }

            let host = domain.unwrap_or(&existing.ip);
            eprintln!("Deploy with:");
            eprintln!("  cargo xtask deploy {host}");
            return Ok(());
        }

        // Detect SSH keys
        let keys = provisioner.detect_ssh_keys()?;
        let key_ids: Vec<String> = keys.iter().map(|(id, _)| id.clone()).collect();

        let region = region.unwrap_or("fra1");

        // Setup DNS before server setup so the domain resolves
        // by the time Caddy requests a TLS certificate
        let server = provisioner.create_server(name, region, &key_ids)?;

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

    fn cmd_deploy(
        &self,
        host: &str,
        skip_build: bool,
        dry_run: bool,
        only: &[String],
    ) -> DeployResult<()> {
        if dry_run {
            return self.cmd_deploy_dry_run(host, only);
        }

        let deployer = self
            .deployer
            .as_ref()
            .ok_or_else(|| DeployError::Other("no deployer configured".into()))?;

        // Validate --only names against configured apps
        self.validate_only(only)?;

        // Select which apps to build/transfer
        let selected = self.selected_apps(only);

        if !skip_build {
            for app in &selected {
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
            let caddyfile_content = caddyfile::render(&self.caddy, host);
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
            // Only stop selected app containers, keep Caddy
            let stop_names: Vec<&str> = selected.iter().map(|a| a.name.as_str()).collect();
            let names = stop_names.join(" ");
            ssh.exec(&format!(
                "cd {} && docker compose rm -sf {} \
                 2>/dev/null || true",
                self.remote_dir, names,
            ))?;
        } else if only.is_empty() {
            ssh.exec(&format!(
                "cd {} && docker compose down \
                 2>/dev/null || true",
                self.remote_dir
            ))?;
        } else {
            // Only stop selected services
            let stop_names: Vec<&str> = selected.iter().map(|a| a.name.as_str()).collect();
            let names = stop_names.join(" ");
            ssh.exec(&format!(
                "cd {} && docker compose rm -sf {} \
                 2>/dev/null || true",
                self.remote_dir, names,
            ))?;
        }

        for app in &selected {
            deployer.transfer_image(app, host, &self.ssh_user)?;
        }

        deployer.deploy(
            host,
            &self.ssh_user,
            &self.apps,
            &self.caddy,
            &self.remote_dir,
            only,
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

    fn cmd_deploy_local(
        &self,
        domain: &str,
        skip_build: bool,
        dry_run: bool,
        only: &[String],
    ) -> DeployResult<()> {
        if dry_run {
            return self.cmd_deploy_local_dry_run(domain, only);
        }

        // Validate --only names against configured apps
        self.validate_only(only)?;

        let selected = self.selected_apps(only);
        let deployer = LocalDeploy::new();

        if !skip_build {
            for app in &selected {
                deployer.build_image(app)?;
            }
        }

        // Stop existing local stack
        let compose_path = format!("{}/docker-compose.yml", self.local_dir);
        if std::path::Path::new(&compose_path).exists() {
            if only.is_empty() {
                eprintln!("Stopping existing local stack...");
                let _ = run_local_compose(&self.local_dir, &["down"]);
            } else {
                let names: Vec<&str> = selected.iter().map(|a| a.name.as_str()).collect();
                let name_strs = names.join(" ");
                eprintln!("Stopping selected services: {name_strs}...");
                let mut args = vec!["rm", "-sf"];
                args.extend(names);
                let _ = run_local_compose(&self.local_dir, &args);
            }
        }

        deployer.deploy(domain, "", &self.apps, &self.caddy, &self.local_dir, only)?;

        // Print dnsmasq setup hint if not detected
        print_dnsmasq_hint();

        Ok(())
    }

    fn cmd_local_down(&self) -> DeployResult<()> {
        let compose_path = format!("{}/docker-compose.yml", self.local_dir);
        if !std::path::Path::new(&compose_path).exists() {
            eprintln!("No local stack found in {}/", self.local_dir);
            return Ok(());
        }

        eprintln!("Stopping local stack...");
        run_local_compose(&self.local_dir, &["down"])
    }

    fn cmd_local_status(&self) -> DeployResult<()> {
        let compose_path = format!("{}/docker-compose.yml", self.local_dir);
        if !std::path::Path::new(&compose_path).exists() {
            eprintln!("No local stack found in {}/", self.local_dir);
            return Ok(());
        }

        run_local_compose(&self.local_dir, &["ps"])
    }

    #[allow(clippy::unnecessary_wraps)]
    fn cmd_deploy_dry_run(&self, host: &str, only: &[String]) -> DeployResult<()> {
        self.validate_only(only)?;
        let selected = self.selected_apps(only);

        let compose_content = compose::render(&self.apps, &self.caddy);
        let caddyfile_content = caddyfile::render(&self.caddy, host);

        eprintln!("=== Dry run: no changes will be made ===");
        if !only.is_empty() {
            eprintln!("  (--only: {})", only.join(", "));
        }
        eprintln!();

        eprintln!("--- docker-compose.yml ---");
        println!("{compose_content}");

        eprintln!("--- Caddyfile ---");
        println!("{caddyfile_content}");

        eprintln!("--- Actions that would be performed ---");
        for (i, app) in selected.iter().enumerate() {
            let n = i + 1;
            eprintln!("{n}. Build Docker image: {}:latest", app.name);
        }
        let base = selected.len();
        for (i, app) in selected.iter().enumerate() {
            let n = base + i + 1;
            eprintln!("{n}. Transfer {} to {}@{}", app.name, self.ssh_user, host);
        }
        let mut step = base * 2 + 1;
        eprintln!("{step}. Write config files to {}/", self.remote_dir);
        step += 1;
        let has_env = selected.iter().any(|a| a.env_file.is_some());
        if has_env {
            eprintln!("{step}. Transfer .env file(s)");
            step += 1;
        }
        if only.is_empty() {
            eprintln!("{step}. Restart containers via docker compose");
        } else {
            eprintln!("{step}. Restart services: {}", only.join(", "));
        }

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

    #[allow(clippy::unnecessary_wraps)]
    fn cmd_deploy_local_dry_run(&self, domain: &str, only: &[String]) -> DeployResult<()> {
        self.validate_only(only)?;
        let selected = self.selected_apps(only);

        let compose_content = compose::render(&self.apps, &self.caddy);

        let mut local_caddy = self.caddy.clone();
        local_caddy.tls_internal = true;
        let caddyfile_content = caddyfile::render(&local_caddy, domain);

        eprintln!(
            "=== Dry run (local): \
             no changes will be made ==="
        );
        if !only.is_empty() {
            eprintln!("  (--only: {})", only.join(", "));
        }
        eprintln!();

        eprintln!("--- docker-compose.yml ---");
        println!("{compose_content}");

        eprintln!("--- Caddyfile (tls internal) ---");
        println!("{caddyfile_content}");

        eprintln!("--- Actions that would be performed ---");
        for (i, app) in selected.iter().enumerate() {
            let n = i + 1;
            eprintln!(
                "{n}. Build Docker image (native): \
                 {}:latest",
                app.name
            );
        }
        let mut step = selected.len() + 1;
        eprintln!("{step}. Write config files to {}/", self.local_dir);
        step += 1;
        let has_env = selected.iter().any(|a| a.env_file.is_some());
        if has_env {
            eprintln!("{step}. Copy .env file(s)");
            step += 1;
        }
        if only.is_empty() {
            eprintln!("{step}. Start containers via docker compose");
        } else {
            eprintln!("{step}. Start services: {}", only.join(", "));
        }

        Ok(())
    }

    fn cmd_status(&self, host: &str) -> DeployResult<()> {
        let ssh = SshSession::new(host, &self.ssh_user);
        ssh.exec_interactive(&format!("cd {} && docker compose ps", self.remote_dir))
    }

    fn cmd_destroy(&self, name: &str, force: bool) -> DeployResult<()> {
        let provisioner = self
            .provisioner
            .as_ref()
            .ok_or_else(|| DeployError::Other("no provisioner configured".into()))?;

        // Show what will be destroyed
        eprintln!(
            "WARNING: This will permanently delete \
             droplet '{name}'"
        );
        if !self.dns.is_empty() {
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
        for dns in &self.dns {
            let d = dns.domain();
            eprintln!("Removing DNS record for {d}...");
            dns.delete_a_record()?;
        }

        eprintln!();
        eprintln!("Cleanup complete!");

        Ok(())
    }
}

/// Run `docker compose` with an explicit project directory
/// so relative paths and project naming stay consistent.
fn run_local_compose(local_dir: &str, args: &[&str]) -> DeployResult<()> {
    let compose_file = format!("{local_dir}/docker-compose.yml");
    let mut full: Vec<&str> = vec![
        "compose",
        "--project-directory",
        local_dir,
        "-f",
        &compose_file,
    ];
    full.extend_from_slice(args);
    cmd::run_interactive("docker", &full)
}

/// Print a one-time dnsmasq setup guide when dnsmasq is not
/// running.
fn print_dnsmasq_hint() {
    let running = cmd::run("brew", &["services", "list"])
        .map(|out| out.contains("dnsmasq") && out.contains("started"))
        .unwrap_or(false);

    if running {
        return;
    }

    eprintln!();
    eprintln!("Local DNS not configured. One-time setup:");
    eprintln!();
    eprintln!("  brew install dnsmasq");
    eprintln!("  echo 'address=/.local.dev/127.0.0.1' >> \\");
    eprintln!("    /opt/homebrew/etc/dnsmasq.conf");
    eprintln!("  sudo mkdir -p /etc/resolver");
    eprintln!("  echo 'nameserver 127.0.0.1' | \\");
    eprintln!("    sudo tee /etc/resolver/local.dev");
    eprintln!("  brew services start dnsmasq");
    eprintln!();
    eprintln!(
        "Then use domains like myapp.local.dev \
         for local deploys."
    );
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

        /// Deploy only the listed services (repeatable)
        #[arg(long)]
        only: Vec<String>,
    },

    /// Deploy locally for testing
    DeployLocal {
        /// Domain name for local access
        domain: String,

        /// Skip Docker image build
        #[arg(long)]
        skip_build: bool,

        /// Preview generated files without executing
        #[arg(long)]
        dry_run: bool,

        /// Deploy only the listed services (repeatable)
        #[arg(long)]
        only: Vec<String>,
    },

    /// Stop the local stack
    LocalDown,

    /// Show local container status
    LocalStatus,

    /// Show container status on a remote server
    Status {
        /// Hostname or IP address
        host: String,
    },

    /// Destroy a server
    Destroy {
        /// Server name
        name: String,

        /// Skip interactive confirmation prompt
        #[arg(long)]
        force: bool,
    },
}
