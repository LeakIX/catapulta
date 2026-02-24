pub mod digitalocean;

use crate::error::DeployResult;

/// Information about a provisioned server.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub ip: String,
    pub region: String,
    pub ssh_key_id: String,
    pub ssh_key_file: String,
}

/// A provisioner creates, configures, and destroys cloud servers.
pub trait Provisioner {
    /// Check that all prerequisites are installed and
    /// authenticated.
    fn check_prerequisites(&self) -> DeployResult<()>;

    /// Create a new server and return its info.
    fn create_server(&self, name: &str, region: &str, ssh_key_id: &str)
    -> DeployResult<ServerInfo>;

    /// Install Docker, configure firewall, start Caddy
    /// placeholder.
    fn setup_server(&self, server: &ServerInfo, domain: Option<&str>) -> DeployResult<()>;

    /// Get an existing server by name.
    fn get_server(&self, name: &str) -> DeployResult<Option<ServerInfo>>;

    /// Destroy a server by name.
    fn destroy_server(&self, name: &str) -> DeployResult<()>;
}
