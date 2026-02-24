use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::app::App;
use crate::caddy::Caddy;
use crate::caddyfile;
use crate::cmd;
use crate::compose;
use crate::deploy::Deployer;
use crate::error::{DeployError, DeployResult};
use crate::ssh::SshSession;

/// Deploy via `docker save | gzip | ssh | docker load`.
///
/// This is the simplest deployment strategy - no registry
/// needed. The image is built locally for linux/amd64,
/// streamed over SSH, then started with docker compose.
pub struct DockerSaveLoad;

impl DockerSaveLoad {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn check_env_file(app: &App) -> DeployResult<()> {
        if let Some(env_file) = &app.env_file {
            if !Path::new(env_file).exists() {
                return Err(DeployError::FileNotFound(format!(
                    "{env_file} not found. \
                     Create from .env.example"
                )));
            }
        }
        Ok(())
    }
}

impl Default for DockerSaveLoad {
    fn default() -> Self {
        Self::new()
    }
}

impl Deployer for DockerSaveLoad {
    fn build_image(&self, app: &App) -> DeployResult<()> {
        eprintln!("Building Docker image for {}...", app.platform);

        let mut args = vec!["build", "--platform", &app.platform, "-f", &app.dockerfile];

        let build_arg_strings: Vec<String> = app
            .build_args
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();

        for arg_str in &build_arg_strings {
            args.push("--build-arg");
            args.push(arg_str);
        }

        let tag = format!("{}:latest", app.name);
        args.push("-t");
        args.push(&tag);
        args.push(".");

        cmd::run_interactive("docker", &args)
    }

    fn transfer_image(&self, app: &App, host: &str, user: &str) -> DeployResult<()> {
        let tag = format!("{}:latest", app.name);

        // Query image size for logging and progress
        let size_bytes = cmd::run(
            "docker",
            &["image", "inspect", "--format", "{{.Size}}", &tag],
        )?;
        let size_bytes: u64 = size_bytes.parse().unwrap_or(0);
        let size_mb = size_bytes / (1024 * 1024);

        eprintln!("Transferring image {tag} ({size_mb} MB) to {user}@{host}");

        // Use pv for a progress bar when available, plain pipe otherwise
        let progress = if cmd::command_exists("pv") {
            format!("pv -s {size_bytes} -p -t -e -r -b")
        } else {
            "cat".to_string()
        };

        eprintln!("  Saving image, compressing, and streaming over SSH...");
        let pipeline = format!(
            "docker save {tag} | {progress} | gzip | \
             ssh {user}@{host} 'gunzip | docker load'"
        );
        cmd::run_pipeline(&pipeline)?;

        eprintln!("  Image loaded on {host}");
        Ok(())
    }

    fn deploy(
        &self,
        host: &str,
        user: &str,
        app: &App,
        caddy: &Caddy,
        remote_dir: &str,
    ) -> DeployResult<()> {
        Self::check_env_file(app)?;

        eprintln!("Deploying to {user}@{host}...");

        let ssh = SshSession::new(host, user);

        // Generate Caddyfile
        let caddyfile_content = caddyfile::render(caddy, host);

        // Generate docker-compose.yml
        let compose_content = compose::render(app, caddy);

        // Write generated files to remote
        eprintln!("Writing deployment config...");
        ssh.write_remote_file(
            &compose_content,
            &format!("{remote_dir}/docker-compose.yml"),
        )?;
        ssh.write_remote_file(&caddyfile_content, &format!("{remote_dir}/Caddyfile"))?;

        // Transfer .env file if specified
        if let Some(env_file) = &app.env_file {
            ssh.scp_to(env_file, &format!("{remote_dir}/.env"))?;
            ssh.exec(&format!("chmod 600 {remote_dir}/.env"))?;
        }

        // Restart containers
        eprintln!("Starting containers...");
        ssh.exec_interactive(&format!(
            "cd {remote_dir} && \
             docker compose down 2>/dev/null || true && \
             docker compose up -d"
        ))?;

        // Wait for health
        wait_healthy(&ssh, app, remote_dir)?;

        // Show status
        ssh.exec_interactive(
            &format!("cd {remote_dir} && docker compose ps"),
        )?;

        eprintln!();
        eprintln!("Deployment complete!");
        eprintln!("Application available at: https://{host}");

        Ok(())
    }
}

/// Poll container health status instead of sleeping a fixed
/// duration. When the app has a healthcheck configured, queries
/// `docker inspect` in a loop. Falls back to a brief sleep when
/// no healthcheck is defined.
fn wait_healthy(
    ssh: &SshSession,
    app: &App,
    remote_dir: &str,
) -> DeployResult<()> {
    const MAX_ATTEMPTS: u32 = 30;
    const INTERVAL: Duration = Duration::from_secs(5);

    if app.healthcheck.is_none() {
        eprintln!("No healthcheck configured, waiting 5s...");
        thread::sleep(Duration::from_secs(5));
        return Ok(());
    }

    eprintln!("Waiting for container to be healthy...");

    for attempt in 1..=MAX_ATTEMPTS {
        let output = ssh.exec(&format!(
            "cd {remote_dir} && \
             docker inspect \
             --format='{{{{.State.Health.Status}}}}' {}",
            app.name
        ));

        match output {
            Ok(status) => {
                let status = status.trim();
                eprint!(
                    "  Health check ({attempt}/{MAX_ATTEMPTS}): \
                     {status}"
                );
                if status == "healthy" {
                    eprintln!();
                    return Ok(());
                }
                eprintln!(" - retrying...");
            }
            Err(_) => {
                eprintln!(
                    "  Health check ({attempt}/{MAX_ATTEMPTS}): \
                     waiting for container..."
                );
            }
        }

        thread::sleep(INTERVAL);
    }

    Err(DeployError::HealthcheckTimeout(
        app.name.clone(),
        MAX_ATTEMPTS,
    ))
}
