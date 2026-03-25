use std::fs;

use crate::app::App;
use crate::caddy::Caddy;
use crate::caddyfile;
use crate::cmd;
use crate::compose;
use crate::deploy::{Deployer, check_env_files, cleanup_source, prepare_source, wait_healthy};
use crate::error::DeployResult;

/// Deploy to the local Docker daemon for testing.
///
/// Images are built for the native platform (no cross-compile
/// overhead), and the full compose stack runs locally with
/// `tls internal` for self-signed HTTPS.
///
/// This is a unit struct like [`super::docker_save::DockerSaveLoad`].
/// The local directory is passed as the `remote_dir` parameter
/// to [`Deployer::deploy`].
pub struct LocalDeploy;

impl LocalDeploy {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LocalDeploy {
    fn default() -> Self {
        Self::new()
    }
}

/// Run `docker compose` with an explicit project directory
/// so that relative volume mounts and project naming are
/// consistent regardless of the caller's working directory.
fn compose_cmd(local_dir: &str, args: &[&str]) -> Vec<String> {
    let mut full: Vec<String> = vec![
        "compose".into(),
        "--project-directory".into(),
        local_dir.into(),
        "-f".into(),
        format!("{local_dir}/docker-compose.yml"),
    ];
    full.extend(args.iter().map(|s| (*s).to_string()));
    full
}

fn run_compose(local_dir: &str, args: &[&str]) -> DeployResult<()> {
    let full = compose_cmd(local_dir, args);
    let refs: Vec<&str> = full.iter().map(String::as_str).collect();
    cmd::run_interactive("docker", &refs)
}

impl Deployer for LocalDeploy {
    fn build_image(&self, app: &App) -> DeployResult<()> {
        eprintln!("Building Docker image for native platform...");

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

        // No --platform flag: use native architecture
        let mut args = vec!["build", "-f", &dockerfile];

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

    fn transfer_image(&self, _app: &App, _host: &str, _user: &str) -> DeployResult<()> {
        // No-op: images are already in the local daemon
        Ok(())
    }

    fn deploy(
        &self,
        host: &str,
        _user: &str,
        apps: &[App],
        caddy: &Caddy,
        local_dir: &str,
        only: &[String],
    ) -> DeployResult<()> {
        // Filter apps for env copy when --only is set
        let env_apps: Vec<&App> = if only.is_empty() {
            apps.iter().collect()
        } else {
            apps.iter().filter(|a| only.contains(&a.name)).collect()
        };

        check_env_files(apps)?;

        eprintln!("Deploying locally to {local_dir}/...");

        // Create local directory
        fs::create_dir_all(local_dir)?;

        // Generate config files with tls internal (always full)
        let mut local_caddy = caddy.clone();
        local_caddy.tls_internal = true;
        let caddyfile_content = caddyfile::render(&local_caddy, host);
        let compose_content = compose::render(apps, caddy);

        // Write config files
        eprintln!("Writing deployment config...");
        fs::write(format!("{local_dir}/docker-compose.yml"), &compose_content)?;
        fs::write(format!("{local_dir}/Caddyfile"), &caddyfile_content)?;

        // Copy .env files (only selected apps)
        for app in &env_apps {
            if let Some(env_file) = &app.env_file {
                let local_name = if apps.len() > 1 {
                    format!("{local_dir}/.env.{}", app.name)
                } else {
                    format!("{local_dir}/.env")
                };
                fs::copy(env_file, &local_name)?;
            }
        }

        // Start containers
        eprintln!("Starting containers...");
        if only.is_empty() {
            run_compose(local_dir, &["up", "-d"])?;
        } else {
            let mut args: Vec<&str> = vec!["up", "-d"];
            let names: Vec<&str> = only.iter().map(String::as_str).collect();
            args.extend(&names);
            run_compose(local_dir, &args)?;
        }

        // Wait for health (only selected apps)
        let health_apps: Vec<App> = env_apps.iter().copied().cloned().collect();
        wait_healthy(&health_apps, |name| {
            cmd::run(
                "docker",
                &["inspect", "--format={{.State.Health.Status}}", name],
            )
        })?;

        // Show status
        run_compose(local_dir, &["ps"])?;

        eprintln!();
        eprintln!("Local deployment complete!");
        eprintln!("Application available at: https://{host}");

        Ok(())
    }
}
