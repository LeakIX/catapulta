use crate::app::App;
use crate::caddy::Caddy;
use crate::cmd;
use crate::deploy::Deployer;
use crate::error::DeployResult;

/// Deploy a static site to Cloudflare Pages via `wrangler`.
///
/// Requires `wrangler` on `PATH` and the `CLOUDFLARE_API_TOKEN`
/// environment variable set with a token that has Pages
/// permissions.
///
/// # Example
///
/// ```rust
/// use catapulta::CloudflarePages;
/// use catapulta::deploy::Deployer;
///
/// let deployer = CloudflarePages::new("my-project");
/// assert_eq!(deployer.cname_target(), Some("my-project.pages.dev".into()));
/// ```
pub struct CloudflarePages {
    project: String,
}

impl CloudflarePages {
    #[must_use]
    pub fn new(project: &str) -> Self {
        Self {
            project: project.to_string(),
        }
    }
}

impl Deployer for CloudflarePages {
    fn build_image(&self, app: &App) -> DeployResult<()> {
        if let Some(build_cmd) = &app.build_cmd {
            eprintln!("Building static site for {}...", app.name);
            cmd::run_interactive("sh", &["-c", build_cmd])?;
        }
        Ok(())
    }

    fn transfer_image(&self, _app: &App, _host: &str, _user: &str) -> DeployResult<()> {
        // No transfer needed for Cloudflare Pages.
        Ok(())
    }

    fn deploy(
        &self,
        _host: &str,
        _user: &str,
        apps: &[App],
        _caddy: &Caddy,
        _remote_dir: &str,
    ) -> DeployResult<()> {
        for app in apps {
            let build_dir = app.build_dir.as_deref().unwrap_or("dist");

            eprintln!(
                "Deploying {} to Cloudflare Pages project '{}'...",
                app.name, self.project
            );

            cmd::run_interactive(
                "wrangler",
                &[
                    "pages",
                    "deploy",
                    build_dir,
                    "--project-name",
                    &self.project,
                ],
            )?;

            eprintln!("Deployed to https://{}.pages.dev", self.project);
        }

        Ok(())
    }

    fn is_remote(&self) -> bool {
        false
    }

    fn cname_target(&self) -> Option<String> {
        Some(format!("{}.pages.dev", self.project))
    }
}
