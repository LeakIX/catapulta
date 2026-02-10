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
        app: &App,
        caddy: &Caddy,
        remote_dir: &str,
    ) -> DeployResult<()>;
}
