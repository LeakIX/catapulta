use std::net::Ipv4Addr;

use cloudflare::endpoints::dns::dns::{
    CreateDnsRecord, CreateDnsRecordParams, DeleteDnsRecord, DnsContent, ListDnsRecords,
    ListDnsRecordsParams, UpdateDnsRecord, UpdateDnsRecordParams,
};
use cloudflare::endpoints::zones::zone::{ListZones, ListZonesParams};
use cloudflare::framework::Environment;
use cloudflare::framework::auth::Credentials;
use cloudflare::framework::client::ClientConfig;
use cloudflare::framework::client::async_api::Client;

use crate::dns::{self, DnsProvider};
use crate::error::{DeployError, DeployResult};

/// Cloudflare DNS provider using the official cloudflare crate.
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

    fn client(token: &str) -> DeployResult<Client> {
        Client::new(
            Credentials::UserAuthToken {
                token: token.to_string(),
            },
            ClientConfig::default(),
            Environment::Production,
        )
        .map_err(|e| DeployError::DnsError(e.to_string()))
    }

    fn block_on<F: std::future::Future>(f: F) -> DeployResult<F::Output> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| DeployError::DnsError(e.to_string()))
            .map(|rt| rt.block_on(f))
    }

    fn get_zone_id(client: &Client, zone: &str) -> DeployResult<String> {
        let response = Self::block_on(client.request(&ListZones {
            params: ListZonesParams {
                name: Some(zone.to_string()),
                ..ListZonesParams::default()
            },
        }))?
        .map_err(|e| DeployError::DnsError(e.to_string()))?;

        response
            .result
            .first()
            .map(|z| z.id.clone())
            .ok_or_else(|| DeployError::DnsError(format!("zone '{zone}' not found")))
    }

    fn find_existing_record(
        client: &Client,
        zone_id: &str,
        domain: &str,
    ) -> DeployResult<Option<String>> {
        let response = Self::block_on(client.request(&ListDnsRecords {
            zone_identifier: zone_id,
            params: ListDnsRecordsParams {
                name: Some(domain.to_string()),
                record_type: Some(DnsContent::A {
                    content: Ipv4Addr::UNSPECIFIED,
                }),
                ..ListDnsRecordsParams::default()
            },
        }))?
        .map_err(|e| DeployError::DnsError(e.to_string()))?;

        Ok(response.result.first().map(|r| r.id.clone()))
    }
}

impl DnsProvider for Cloudflare {
    fn domain(&self) -> &str {
        &self.domain
    }

    fn upsert_a_record(&self, ip: &str) -> DeployResult<()> {
        let token = Self::token()?;
        let client = Self::client(&token)?;
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

        let ip_addr: Ipv4Addr = ip
            .parse()
            .map_err(|e| DeployError::DnsError(format!("invalid IP: {e}")))?;

        let zone_id = Self::get_zone_id(&client, &zone)?;
        let existing = Self::find_existing_record(&client, &zone_id, &self.domain)?;

        if let Some(record_id) = existing {
            eprintln!("  Updating existing A record...");
            Self::block_on(client.request(&UpdateDnsRecord {
                zone_identifier: &zone_id,
                identifier: &record_id,
                params: UpdateDnsRecordParams {
                    ttl: Some(300),
                    proxied: Some(false),
                    name: &self.domain,
                    content: DnsContent::A { content: ip_addr },
                },
            }))?
            .map_err(|e| DeployError::DnsError(e.to_string()))?;
        } else {
            eprintln!("  Creating new A record...");
            Self::block_on(client.request(&CreateDnsRecord {
                zone_identifier: &zone_id,
                params: CreateDnsRecordParams {
                    ttl: Some(300),
                    priority: None,
                    proxied: Some(false),
                    name: &self.domain,
                    content: DnsContent::A { content: ip_addr },
                },
            }))?
            .map_err(|e| DeployError::DnsError(e.to_string()))?;
        }

        eprintln!("DNS record set: {} -> {ip}", self.domain);
        Ok(())
    }

    fn delete_a_record(&self) -> DeployResult<()> {
        let token = Self::token()?;
        let client = Self::client(&token)?;
        let (zone, _) = dns::split_domain(&self.domain);

        let zone_id = Self::get_zone_id(&client, &zone)?;
        let existing = Self::find_existing_record(&client, &zone_id, &self.domain)?;

        if let Some(record_id) = existing {
            eprintln!("  Deleting A record...");
            Self::block_on(client.request(&DeleteDnsRecord {
                zone_identifier: &zone_id,
                identifier: &record_id,
            }))?
            .map_err(|e| DeployError::DnsError(e.to_string()))?;
            eprintln!("DNS record deleted: {}", self.domain);
        } else {
            eprintln!("No A record found for {}", self.domain);
        }

        Ok(())
    }
}
