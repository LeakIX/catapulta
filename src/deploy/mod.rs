pub mod cloudflare_pages;
pub mod docker_save;

use crate::app::App;
use crate::caddy::Caddy;
use crate::error::DeployResult;

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

    /// Whether this deployer targets a remote host via SSH.
    ///
    /// Returns `false` for local deployers like Cloudflare Pages
    /// that do not need SSH access.
    fn is_remote(&self) -> bool {
        true
    }

    /// CNAME target for DNS setup (e.g. `"project.pages.dev"`).
    ///
    /// Non-remote deployers may return a value so the pipeline
    /// can create CNAME records automatically.
    fn cname_target(&self) -> Option<String> {
        None
    }
}
