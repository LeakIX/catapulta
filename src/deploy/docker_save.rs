use crate::app::App;
use crate::caddy::Caddy;
use crate::caddyfile;
use crate::cmd;
use crate::compose;
use crate::deploy::{Deployer, check_env_files, cleanup_source, prepare_source, wait_healthy};
use crate::error::DeployResult;
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
        only: &[String],
    ) -> DeployResult<()> {
        // Filter apps for env transfer when --only is set
        let env_apps: Vec<&App> = if only.is_empty() {
            apps.iter().collect()
        } else {
            apps.iter().filter(|a| only.contains(&a.name)).collect()
        };

        check_env_files(apps)?;

        eprintln!("Deploying to {user}@{host}...");

        let ssh = SshSession::new(host, user);

        // Generate config files (always full stack)
        let caddyfile_content = caddyfile::render(caddy, host);
        let compose_content = compose::render(apps, caddy);

        // Write generated files to remote
        eprintln!("Writing deployment config...");
        ssh.write_remote_file(
            &compose_content,
            &format!("{remote_dir}/docker-compose.yml"),
        )?;
        ssh.write_remote_file(&caddyfile_content, &format!("{remote_dir}/Caddyfile"))?;

        // Transfer .env files (only selected apps)
        for app in &env_apps {
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

        // Start containers
        eprintln!("Starting containers...");
        if only.is_empty() {
            ssh.exec_interactive(&format!("cd {remote_dir} && docker compose up -d"))?;
        } else {
            let names = only.join(" ");
            ssh.exec_interactive(&format!(
                "cd {remote_dir} && \
                 docker compose up -d {names}"
            ))?;
        }

        // Wait for health (only selected apps)
        let health_apps: Vec<App> = env_apps.iter().map(|a| (*a).clone()).collect();
        let rd = remote_dir.to_string();
        wait_healthy(&health_apps, |name| {
            ssh.exec(&format!(
                "cd {rd} && \
                     docker inspect \
                     --format='{{{{.State.Health.Status}}}}' \
                     {name}"
            ))
        })?;

        // Show status
        ssh.exec_interactive(&format!("cd {remote_dir} && docker compose ps"))?;

        eprintln!();
        eprintln!("Deployment complete!");
        eprintln!("Application available at: https://{host}");

        Ok(())
    }
}
