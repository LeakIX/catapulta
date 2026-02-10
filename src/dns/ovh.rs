use std::fs;
use std::path::PathBuf;

use crate::cmd;
use crate::dns::{self, DnsProvider};
use crate::error::{DeployError, DeployResult};

/// OVH DNS provider using the OVH REST API via curl.
///
/// Reads credentials from `~/.ovh.conf` (written by
/// `ovhcloud login`).
pub struct Ovh {
    /// The fully-qualified domain name to manage.
    pub domain: String,
}

/// Credentials read from `~/.ovh.conf`.
pub struct OvhCredentials {
    /// OVH endpoint name (e.g. `ovh-eu`).
    pub endpoint: String,
    /// Application key.
    pub application_key: String,
    /// Application secret.
    pub application_secret: String,
    /// Consumer key.
    pub consumer_key: String,
}

impl Ovh {
    #[must_use]
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
        }
    }

    fn read_credentials() -> DeployResult<OvhCredentials> {
        let home = std::env::var("HOME").map_err(|_| DeployError::EnvMissing("HOME".into()))?;
        let conf_path = PathBuf::from(home).join(".ovh.conf");

        if !conf_path.exists() {
            return Err(DeployError::FileNotFound(
                "~/.ovh.conf not found. Run: ovhcloud login".into(),
            ));
        }

        let content = fs::read_to_string(&conf_path)?;
        let endpoint = parse_ini_value(&content, "default", "endpoint")
            .ok_or_else(|| DeployError::Other("missing endpoint in ~/.ovh.conf".into()))?;

        let ak = parse_ini_value(&content, &endpoint, "application_key")
            .ok_or_else(|| DeployError::Other("missing application_key in ~/.ovh.conf".into()))?;

        let app_secret =
            parse_ini_value(&content, &endpoint, "application_secret").ok_or_else(|| {
                DeployError::Other("missing application_secret in ~/.ovh.conf".into())
            })?;

        let ck = parse_ini_value(&content, &endpoint, "consumer_key")
            .ok_or_else(|| DeployError::Other("missing consumer_key in ~/.ovh.conf".into()))?;

        Ok(OvhCredentials {
            endpoint,
            application_key: ak,
            application_secret: app_secret,
            consumer_key: ck,
        })
    }

    /// Map an OVH endpoint name to its API base URL.
    #[must_use]
    pub fn api_base(creds: &OvhCredentials) -> String {
        // Map endpoint names to API bases
        match creds.endpoint.as_str() {
            "ovh-eu" => "https://eu.api.ovh.com/1.0".to_string(),
            "ovh-us" => "https://api.us.ovhcloud.com/1.0".to_string(),
            "ovh-ca" => "https://ca.api.ovh.com/1.0".to_string(),
            other => format!("https://{other}.api.ovh.com/1.0"),
        }
    }

    /// Make a signed OVH API request via curl.
    fn api_request(
        creds: &OvhCredentials,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> DeployResult<String> {
        let base = Self::api_base(creds);
        let url = format!("{base}{path}");

        // Get server timestamp
        let ts = cmd::run("curl", &["-s", &format!("{base}/auth/time")])?;

        // Build signature:
        // $1$SHA1(AS+CK+METHOD+URL+BODY+TS)
        let body_str = body.unwrap_or("");
        let sig_data = format!(
            "{}+{}+{method}+{url}+{body_str}+{ts}",
            creds.application_secret, creds.consumer_key,
        );

        let sha1 = cmd::run(
            "sh",
            &[
                "-c",
                &format!("printf '%s' '{sig_data}' | shasum -a 1 | cut -d' ' -f1"),
            ],
        )?;
        let signature = format!("$1${sha1}");

        let mut args = vec![
            "-s".to_string(),
            "-X".to_string(),
            method.to_string(),
            "-H".to_string(),
            format!("X-Ovh-Application: {}", creds.application_key),
            "-H".to_string(),
            format!("X-Ovh-Consumer: {}", creds.consumer_key),
            "-H".to_string(),
            format!("X-Ovh-Timestamp: {ts}"),
            "-H".to_string(),
            format!("X-Ovh-Signature: {signature}"),
            "-H".to_string(),
            "Content-Type: application/json".to_string(),
        ];

        if let Some(b) = body {
            args.push("-d".to_string());
            args.push(b.to_string());
        }

        args.push(url);

        let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
        cmd::run("curl", &args_ref)
    }
}

impl DnsProvider for Ovh {
    fn domain(&self) -> &str {
        &self.domain
    }

    fn upsert_a_record(&self, ip: &str) -> DeployResult<()> {
        let creds = Self::read_credentials()?;
        let (zone, subdomain) = dns::split_domain(&self.domain);

        eprintln!("OVH DNS: {} -> {ip}", self.domain);
        eprintln!("  Zone: {zone}");
        eprintln!(
            "  SubDomain: {}",
            if subdomain.is_empty() {
                "@"
            } else {
                &subdomain
            }
        );

        // Find existing A record
        let path = format!(
            "/domain/zone/{zone}/record\
             ?fieldType=A&subDomain={subdomain}"
        );
        let response = Self::api_request(&creds, "GET", &path, None)?;

        let ids: Vec<u64> = serde_json::from_str(&response).unwrap_or_default();

        if let Some(record_id) = ids.first() {
            eprintln!("  Updating existing A record (id: {record_id})...");
            let path = format!("/domain/zone/{zone}/record/{record_id}");
            let body = format!(r#"{{"target":"{ip}","ttl":300}}"#);
            Self::api_request(&creds, "PUT", &path, Some(&body))?;
        } else {
            eprintln!("  Creating new A record...");
            let path = format!("/domain/zone/{zone}/record");
            let body = format!(
                r#"{{"fieldType":"A","subDomain":"{subdomain}","target":"{ip}","ttl":300}}"#
            );
            Self::api_request(&creds, "POST", &path, Some(&body))?;
        }

        // Refresh zone
        eprintln!("  Refreshing DNS zone...");
        Self::api_request(
            &creds,
            "POST",
            &format!("/domain/zone/{zone}/refresh"),
            None,
        )?;

        eprintln!("DNS record set: {} -> {ip}", self.domain);
        Ok(())
    }

    fn delete_a_record(&self) -> DeployResult<()> {
        let creds = Self::read_credentials()?;
        let (zone, subdomain) = dns::split_domain(&self.domain);

        let path = format!(
            "/domain/zone/{zone}/record\
             ?fieldType=A&subDomain={subdomain}"
        );
        let response = Self::api_request(&creds, "GET", &path, None)?;

        let ids: Vec<u64> = serde_json::from_str(&response).unwrap_or_default();

        for record_id in &ids {
            eprintln!("  Deleting A record (id: {record_id})...");
            let path = format!("/domain/zone/{zone}/record/{record_id}");
            Self::api_request(&creds, "DELETE", &path, None)?;
        }

        // Refresh zone
        Self::api_request(
            &creds,
            "POST",
            &format!("/domain/zone/{zone}/refresh"),
            None,
        )?;

        eprintln!("DNS record deleted: {}", self.domain);
        Ok(())
    }
}

/// Parse a value from an INI-style config file.
///
/// Looks for `[section]`, then finds `key = value` within that
/// section.
/// Extract a value from an INI-style config string.
#[must_use]
pub fn parse_ini_value(content: &str, section: &str, key: &str) -> Option<String> {
    let section_header = format!("[{section}]");
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            if in_section {
                return None;
            }
            continue;
        }
        if in_section {
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}
