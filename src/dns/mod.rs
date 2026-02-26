pub mod cloudflare;
pub mod ovh;

use crate::error::{DeployError, DeployResult};

/// A DNS provider that can create, update, and delete records.
pub trait DnsProvider {
    /// The fully-qualified domain name managed by this provider.
    fn domain(&self) -> &str;

    /// Create or update an A record pointing to `ip`.
    fn upsert_a_record(&self, ip: &str) -> DeployResult<()>;

    /// Delete the A record for this domain.
    fn delete_a_record(&self) -> DeployResult<()>;

    /// Create or update a CNAME record pointing to `target`.
    fn upsert_cname_record(&self, target: &str) -> DeployResult<()> {
        let _ = target;
        Err(DeployError::Other(
            "CNAME records not supported by this provider".into(),
        ))
    }

    /// Delete the CNAME record for this domain.
    fn delete_cname_record(&self) -> DeployResult<()> {
        Err(DeployError::Other(
            "CNAME records not supported by this provider".into(),
        ))
    }
}

/// Split an FQDN into (zone, subdomain).
///
/// Example: `"app.example.com"` -> `("example.com", "app")`
///
/// If the domain has no subdomain (e.g. `"example.com"`), the
/// subdomain is returned as an empty string.
#[must_use]
pub fn split_domain(fqdn: &str) -> (String, String) {
    let parts: Vec<&str> = fqdn.split('.').collect();
    if parts.len() <= 2 {
        return (fqdn.to_string(), String::new());
    }
    let zone = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    let subdomain = parts[..parts.len() - 2].join(".");
    (zone, subdomain)
}
