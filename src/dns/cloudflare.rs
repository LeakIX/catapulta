use crate::cmd;
use crate::dns::{self, DnsProvider};
use crate::error::{DeployError, DeployResult};

const CF_API: &str = "https://api.cloudflare.com/client/v4";

/// Cloudflare DNS provider using the Cloudflare API via curl.
///
/// Requires `CF_API_TOKEN` environment variable set with a token
/// that has `Zone > DNS > Edit` permissions.
pub struct Cloudflare {
    domain: String,
}

impl Cloudflare {
    #[must_use]
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
        }
    }

    fn token() -> DeployResult<String> {
        std::env::var("CF_API_TOKEN").map_err(|_| {
            DeployError::EnvMissing(
                "CF_API_TOKEN not set. Create a token at: \
                 https://dash.cloudflare.com/profile/api-tokens"
                    .into(),
            )
        })
    }

    fn api_get(token: &str, path: &str) -> DeployResult<String> {
        let url = format!("{CF_API}{path}");
        cmd::run(
            "curl",
            &[
                "-s",
                "-X",
                "GET",
                "-H",
                &format!("Authorization: Bearer {token}"),
                "-H",
                "Content-Type: application/json",
                &url,
            ],
        )
    }

    fn api_request(
        token: &str,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> DeployResult<String> {
        let url = format!("{CF_API}{path}");
        let mut args = vec![
            "-s".to_string(),
            "-X".to_string(),
            method.to_string(),
            "-H".to_string(),
            format!("Authorization: Bearer {token}"),
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

    fn get_zone_id(token: &str, zone: &str) -> DeployResult<String> {
        let path = format!("/zones?name={zone}");
        let response = Self::api_get(token, &path)?;
        let parsed: serde_json::Value = serde_json::from_str(&response)?;

        parsed["result"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|z| z["id"].as_str())
            .map(String::from)
            .ok_or_else(|| DeployError::DnsError(format!("zone '{zone}' not found")))
    }

    fn find_existing_record(
        token: &str,
        zone_id: &str,
        domain: &str,
    ) -> DeployResult<Option<String>> {
        let path = format!("/zones/{zone_id}/dns_records?type=A&name={domain}");
        let response = Self::api_get(token, &path)?;
        let parsed: serde_json::Value = serde_json::from_str(&response)?;

        Ok(parsed["result"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|r| r["id"].as_str())
            .map(String::from))
    }
}

impl DnsProvider for Cloudflare {
    fn domain(&self) -> &str {
        &self.domain
    }

    fn upsert_a_record(&self, ip: &str) -> DeployResult<()> {
        let token = Self::token()?;
        let (zone, subdomain) = dns::split_domain(&self.domain);

        eprintln!("Cloudflare DNS: {} -> {ip}", self.domain);
        eprintln!("  Zone: {zone}");
        eprintln!(
            "  Record: {}",
            if subdomain.is_empty() {
                "@"
            } else {
                &subdomain
            }
        );

        let zone_id = Self::get_zone_id(&token, &zone)?;
        let existing = Self::find_existing_record(&token, &zone_id, &self.domain)?;

        let body = format!(
            r#"{{"type":"A","name":"{}","content":"{ip}","ttl":300,"proxied":false}}"#,
            self.domain
        );

        if let Some(record_id) = existing {
            eprintln!("  Updating existing A record...");
            let path = format!("/zones/{zone_id}/dns_records/{record_id}");
            Self::api_request(&token, "PUT", &path, Some(&body))?;
        } else {
            eprintln!("  Creating new A record...");
            let path = format!("/zones/{zone_id}/dns_records");
            Self::api_request(&token, "POST", &path, Some(&body))?;
        }

        eprintln!("DNS record set: {} -> {ip}", self.domain);
        Ok(())
    }

    fn delete_a_record(&self) -> DeployResult<()> {
        let token = Self::token()?;
        let (zone, _) = dns::split_domain(&self.domain);

        let zone_id = Self::get_zone_id(&token, &zone)?;
        let existing = Self::find_existing_record(&token, &zone_id, &self.domain)?;

        if let Some(record_id) = existing {
            eprintln!("  Deleting A record...");
            let path = format!("/zones/{zone_id}/dns_records/{record_id}");
            Self::api_request(&token, "DELETE", &path, None)?;
            eprintln!("DNS record deleted: {}", self.domain);
        } else {
            eprintln!("No A record found for {}", self.domain);
        }

        Ok(())
    }
}
