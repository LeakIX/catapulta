//! Declarative deployment DSL for Rust.
//!
//! Catapulta lets you provision cloud servers, configure DNS, and
//! deploy Docker containers - all from a typed Rust DSL. No YAML,
//! no shell scripts, no manual SSH.
//!
//! The name comes from Portuguese for *catapult*: launch your
//! application to any server in one command.
//!
//! # Overview
//!
//! A deployment is defined as a [`Pipeline`] that wires together:
//!
//! - An [`App`] describing the Docker container (image, env,
//!   volumes, healthcheck)
//! - A [`Caddy`] reverse proxy config (TLS, basic auth, headers)
//! - A [`Provisioner`](provision::Provisioner) for cloud servers
//!   (e.g. [`DigitalOcean`])
//! - A [`DnsProvider`](dns::DnsProvider) for DNS records (e.g.
//!   [`Ovh`], [`Cloudflare`])
//! - A [`Deployer`](deploy::Deployer) strategy (e.g.
//!   [`DockerSaveLoad`])
//!
//! # Quick start
//!
//! Create an `xtask/src/main.rs` in your project:
//!
//! ```rust,no_run
//! use catapulta::{
//!     App, Caddy, DigitalOcean, DockerSaveLoad, Ovh, Pipeline,
//! };
//!
//! fn main() -> anyhow::Result<()> {
//!     let app = App::new("my-service")
//!         .dockerfile("Dockerfile")
//!         .env("SERVER_HOST", "0.0.0.0")
//!         .env("SERVER_PORT", "3000")
//!         .volume("app-data", "/app/data")
//!         .healthcheck("curl -f http://localhost:3000/")
//!         .expose(3000);
//!
//!     let caddy = Caddy::new()
//!         .reverse_proxy("my-service:3000")
//!         .gzip()
//!         .security_headers();
//!
//!     let pipeline = Pipeline::new(app, caddy)
//!         .provision(DigitalOcean::new())
//!         .dns(Ovh::new("my-service.example.com"))
//!         .deploy(DockerSaveLoad::new());
//!
//!     pipeline.run()?;
//!     Ok(())
//! }
//! ```
//!
//! Then use `cargo xtask` subcommands:
//!
//! ```sh
//! # Provision a new server
//! cargo xtask provision my-service --domain my-service.example.com
//!
//! # Deploy the application
//! cargo xtask deploy my-service.example.com
//!
//! # Tear everything down
//! cargo xtask destroy my-service --domain my-service.example.com
//! ```
//!
//! # Architecture
//!
//! The pipeline follows a three-phase model:
//!
//! 1. **Provision** - create a VPS, install Docker, configure
//!    firewall, set up SSH
//! 2. **DNS** - create or update A records pointing to the server
//! 3. **Deploy** - build the image, transfer it, generate
//!    `docker-compose.yml` and `Caddyfile`, start containers
//!
//! Each phase is pluggable via traits ([`Provisioner`],
//! [`DnsProvider`], [`Deployer`]).
//!
//! [`Provisioner`]: provision::Provisioner
//! [`DnsProvider`]: dns::DnsProvider
//! [`Deployer`]: deploy::Deployer

// Allow noisy pedantic lints that don't add value for a
// deployment tool crate.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
)]

pub mod app;
pub mod caddy;
pub mod caddyfile;
pub mod cmd;
pub mod compose;
pub mod deploy;
pub mod dns;
pub mod error;
pub mod pipeline;
pub mod provision;
pub mod ssh;

pub use app::App;
pub use caddy::Caddy;
pub use deploy::docker_save::DockerSaveLoad;
pub use dns::cloudflare::Cloudflare;
pub use dns::ovh::Ovh;
pub use dns::ovh::OvhCredentials;
pub use dns::ovh::parse_ini_value;
pub use pipeline::Pipeline;
pub use provision::digitalocean::DigitalOcean;
pub use provision::digitalocean::remove_ssh_host_entry;
