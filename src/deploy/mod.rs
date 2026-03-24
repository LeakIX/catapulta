pub mod docker_save;
pub mod local;

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::app::App;
use crate::caddy::Caddy;
use crate::cmd;
use crate::error::{DeployError, DeployResult};

/// A deployer builds, transfers, and starts containers on
/// a remote host.
pub trait Deployer {
    /// Build the Docker image locally.
    fn build_image(&self, app: &App) -> DeployResult<()>;

    /// Transfer the image to the remote host.
    fn transfer_image(&self, app: &App, host: &str, user: &str) -> DeployResult<()>;

    /// Deploy the full stack to the remote host.
    fn deploy(
        &self,
        host: &str,
        user: &str,
        apps: &[App],
        caddy: &Caddy,
        remote_dir: &str,
    ) -> DeployResult<()>;
}

/// Verify that all referenced `.env` files exist on disk.
pub fn check_env_files(apps: &[App]) -> DeployResult<()> {
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

/// Clone a remote Git repository for use as Docker build context.
///
/// Returns `Some(PathBuf)` to the cloned directory when
/// `app.source` is set, or `None` for local builds.
pub fn prepare_source(app: &App) -> DeployResult<Option<PathBuf>> {
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
pub fn cleanup_source(dir: &Path) {
    if let Err(e) = std::fs::remove_dir_all(dir) {
        eprintln!("Warning: failed to clean up {}: {e}", dir.display());
    }
}

/// Poll container health status via `docker inspect`.
///
/// When an app has a healthcheck configured, queries the health
/// status in a loop. Falls back to a brief sleep when no
/// healthcheck is defined.
///
/// The `inspect_fn` closure runs the inspect command and returns
/// the status string. This allows reuse for both SSH-based remote
/// and local Docker deployments.
pub fn wait_healthy<F>(apps: &[App], inspect_fn: F) -> DeployResult<()>
where
    F: Fn(&str) -> DeployResult<String>,
{
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
            let output = inspect_fn(&app.name);

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
