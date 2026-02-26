use std::path::{Path, PathBuf};
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

/// Deploy via `docker save` + `rsync` + `docker load`.
///
/// This is the simplest deployment strategy - no registry
/// needed. The image is built locally for linux/amd64,
/// rsynced to the remote host, then loaded with docker.
pub struct DockerSaveLoad;

impl DockerSaveLoad {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn check_env_files(apps: &[App]) -> DeployResult<()> {
        for app in apps {
            if let Some(env_file) = &app.env_file {
                if !Path::new(env_file).exists() {
                    return Err(DeployError::FileNotFound(format!(
                        "{env_file} not found for \
                             app '{}'. Create from \
                             .env.example",
                        app.name
                    )));
                }
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

        let source_dir = prepare_source(app)?;

        let base = source_dir
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned());

        let context = match (&base, &app.context) {
            (Some(b), Some(sub)) => format!("{b}/{sub}"),
            (Some(b), None) => b.clone(),
            (None, Some(ctx)) => ctx.clone(),
            (None, None) => ".".to_string(),
        };

        let dockerfile = if source_dir.is_some() {
            format!("{context}/{}", app.dockerfile)
        } else {
            app.dockerfile.clone()
        };

        let mut args = vec!["build", "--platform", &app.platform, "-f", &dockerfile];

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
        args.push(&context);

        let result = cmd::run_interactive("docker", &args);

        if !app.cache_source {
            if let Some(dir) = &source_dir {
                cleanup_source(dir);
            }
        }

        result
    }

    fn transfer_image(&self, app: &App, host: &str, user: &str) -> DeployResult<()> {
        let tag = format!("{}:latest", app.name);

        // Query image size for logging
        let size_bytes = cmd::run(
            "docker",
            &["image", "inspect", "--format", "{{.Size}}", &tag],
        )?;
        let size_bytes: u64 = size_bytes.parse().unwrap_or(0);
        let size_mb = size_bytes / (1024 * 1024);

        eprintln!(
            "Transferring image {tag} ({size_mb} MB) \
             to {user}@{host}"
        );

        let local_tar = std::env::temp_dir().join(format!("catapulta-{}.tar", app.name));
        let local_tar_str = local_tar.to_string_lossy().to_string();
        let remote_tar = format!("/tmp/catapulta-{}.tar", app.name);

        // 1. Save image to local temp file
        eprintln!("  Saving image to {local_tar_str}...");
        let save_result = cmd::run_interactive("docker", &["save", &tag, "-o", &local_tar_str]);
        if save_result.is_err() {
            let _ = std::fs::remove_file(&local_tar);
            return save_result;
        }

        // 2. rsync to remote with resume support
        let ssh_cmd = "ssh -o StrictHostKeyChecking=accept-new \
             -o ConnectTimeout=10";
        let dest = format!("{user}@{host}:{remote_tar}");

        eprintln!("  Syncing to {user}@{host}...");
        let rsync_result = cmd::run_interactive(
            "rsync",
            &[
                "-vz",
                "--progress",
                "--partial",
                "-e",
                ssh_cmd,
                &local_tar_str,
                &dest,
            ],
        );
        let _ = std::fs::remove_file(&local_tar);
        rsync_result?;

        // 3. Load on remote and clean up remote tar
        eprintln!("  Loading image on remote...");
        let ssh = SshSession::new(host, user);
        ssh.exec_interactive(&format!(
            "docker load < {remote_tar} && \
             rm -f {remote_tar}"
        ))?;
        eprintln!("  Image loaded on {host}");
        Ok(())
    }

    fn deploy(
        &self,
        host: &str,
        user: &str,
        apps: &[App],
        caddy: &Caddy,
        remote_dir: &str,
    ) -> DeployResult<()> {
        Self::check_env_files(apps)?;

        eprintln!("Deploying to {user}@{host}...");

        let ssh = SshSession::new(host, user);

        // Generate config files
        let caddyfile_content = caddyfile::render(caddy, host);
        let compose_content = compose::render(apps, caddy);

        // Write generated files to remote
        eprintln!("Writing deployment config...");
        ssh.write_remote_file(
            &compose_content,
            &format!("{remote_dir}/docker-compose.yml"),
        )?;
        ssh.write_remote_file(&caddyfile_content, &format!("{remote_dir}/Caddyfile"))?;

        // Transfer .env files for each app
        for app in apps {
            if let Some(env_file) = &app.env_file {
                let remote_name = if apps.len() > 1 {
                    format!("{remote_dir}/.env.{}", app.name)
                } else {
                    format!("{remote_dir}/.env")
                };
                ssh.scp_to(env_file, &remote_name)?;
                ssh.exec(&format!("chmod 600 {remote_name}"))?;
            }
        }

        // Restart containers
        eprintln!("Starting containers...");
        ssh.exec_interactive(&format!(
            "cd {remote_dir} && \
             docker compose down 2>/dev/null || true && \
             docker compose up -d"
        ))?;

        // Wait for health
        wait_healthy(&ssh, apps, remote_dir)?;

        // Show status
        ssh.exec_interactive(&format!("cd {remote_dir} && docker compose ps"))?;

        eprintln!();
        eprintln!("Deployment complete!");
        eprintln!("Application available at: https://{host}");

        Ok(())
    }
}

/// Clone a remote Git repository for use as Docker build context.
///
/// Returns `Some(PathBuf)` to the cloned directory when
/// `app.source` is set, or `None` for local builds.
fn prepare_source(app: &App) -> DeployResult<Option<PathBuf>> {
    let Some((url, git_ref)) = &app.source else {
        return Ok(None);
    };

    if app.cache_source {
        let dir = std::env::temp_dir().join(format!("catapulta-src-{}", app.name));
        let dir_str = dir.to_string_lossy().to_string();

        if dir.exists() {
            eprintln!("Updating cached source for {}...", app.name);
            cmd::run("git", &["-C", &dir_str, "fetch", "origin"])?;
            cmd::run("git", &["-C", &dir_str, "checkout", git_ref])?;
        } else {
            eprintln!("Cloning source for {} (cached)...", app.name);
            cmd::run(
                "git",
                &["clone", "--depth", "1", "--branch", git_ref, url, &dir_str],
            )?;
        }

        Ok(Some(dir))
    } else {
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("catapulta-src-{}-{pid}", app.name));
        let dir_str = dir.to_string_lossy().to_string();

        eprintln!("Cloning source for {}...", app.name);
        cmd::run(
            "git",
            &["clone", "--depth", "1", "--branch", git_ref, url, &dir_str],
        )?;

        Ok(Some(dir))
    }
}

/// Remove a non-cached source directory.
fn cleanup_source(dir: &Path) {
    if let Err(e) = std::fs::remove_dir_all(dir) {
        eprintln!("Warning: failed to clean up {}: {e}", dir.display());
    }
}

/// Poll container health status instead of sleeping a fixed
/// duration. When an app has a healthcheck configured, queries
/// `docker inspect` in a loop. Falls back to a brief sleep when
/// no healthcheck is defined.
fn wait_healthy(ssh: &SshSession, apps: &[App], remote_dir: &str) -> DeployResult<()> {
    const MAX_ATTEMPTS: u32 = 30;
    const INTERVAL: Duration = Duration::from_secs(5);

    let apps_with_hc: Vec<&App> = apps.iter().filter(|a| a.healthcheck.is_some()).collect();

    if apps_with_hc.is_empty() {
        eprintln!("No healthcheck configured, waiting 5s...");
        thread::sleep(Duration::from_secs(5));
        return Ok(());
    }

    eprintln!("Waiting for containers to be healthy...");

    for app in &apps_with_hc {
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
                        "  {} ({attempt}/{MAX_ATTEMPTS}): \
                         {status}",
                        app.name
                    );
                    if status == "healthy" {
                        eprintln!();
                        break;
                    }
                    eprintln!(" - retrying...");
                }
                Err(_) => {
                    eprintln!(
                        "  {} ({attempt}/{MAX_ATTEMPTS}): \
                         waiting for container...",
                        app.name
                    );
                }
            }

            if attempt == MAX_ATTEMPTS {
                return Err(DeployError::HealthcheckTimeout(
                    app.name.clone(),
                    MAX_ATTEMPTS,
                ));
            }

            thread::sleep(INTERVAL);
        }
    }

    Ok(())
}
