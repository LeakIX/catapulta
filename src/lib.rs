//! Declarative deployment DSL for Rust.
//!
//! [Repository](https://github.com/LeakIX/catapulta) |
//! [Online docs](https://leakix.github.io/catapulta/catapulta/) |
//! [crates.io](https://crates.io/crates/catapulta)
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
//! # Examples
//!
//! ## Basic single-app deployment
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
//! # Preview generated files without deploying
//! cargo xtask deploy my-service.example.com --dry-run
//!
//! # Tear everything down
//! cargo xtask destroy my-service --domain my-service.example.com
//! ```
//!
//! ## Multi-app deployment
//!
//! Deploy multiple services behind a single Caddy reverse proxy
//! with path-based routing using [`Pipeline::multi`]:
//!
//! ```rust,no_run
//! use catapulta::{
//!     App, Caddy, DigitalOcean, DockerSaveLoad, Ovh, Pipeline,
//! };
//!
//! fn main() -> anyhow::Result<()> {
//!     let api = App::new("api")
//!         .dockerfile("api/Dockerfile")
//!         .env("DATABASE_URL", "sqlite:/app/data/api.db")
//!         .env_file("deploy/.env.api")
//!         .volume("api-data", "/app/data")
//!         .healthcheck("curl -f http://localhost:8000/health")
//!         .expose(8000);
//!
//!     let web = App::new("web")
//!         .dockerfile("web/Dockerfile")
//!         .env("API_URL", "http://api:8000")
//!         .healthcheck("curl -f http://localhost:3000/")
//!         .expose(3000);
//!
//!     // Caddy routes requests by path:
//!     //   /api/*  ->  api:8000
//!     //   /*      ->  web:3000  (catch-all)
//!     let caddy = Caddy::new()
//!         .route("/api/*", "api:8000")
//!         .route("", "web:3000")
//!         .gzip()
//!         .security_headers();
//!
//!     let pipeline = Pipeline::multi(vec![api, web], caddy)
//!         .provision(DigitalOcean::new().size("s-1vcpu-2gb"))
//!         .dns(Ovh::new("project.example.com"))
//!         .deploy(DockerSaveLoad::new());
//!
//!     pipeline.run()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Cloudflare DNS
//!
//! Use Cloudflare instead of OVH for DNS management.
//! Requires `CF_API_TOKEN` with `Zone > DNS > Edit` permissions.
//!
//! ```rust,no_run
//! use catapulta::{
//!     App, Caddy, Cloudflare, DigitalOcean, DockerSaveLoad,
//!     Pipeline,
//! };
//!
//! fn main() -> anyhow::Result<()> {
//!     let app = App::new("my-service")
//!         .dockerfile("Dockerfile")
//!         .env("PORT", "3000")
//!         .healthcheck("curl -f http://localhost:3000/")
//!         .expose(3000);
//!
//!     let caddy =
//!         Caddy::new().reverse_proxy("my-service:3000").gzip();
//!
//!     let pipeline = Pipeline::new(app, caddy)
//!         .provision(DigitalOcean::new())
//!         .dns(Cloudflare::new("service.example.com"))
//!         .deploy(DockerSaveLoad::new());
//!
//!     pipeline.run()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Basic authentication
//!
//! Protect the application behind HTTP basic auth. The ACME
//! challenge path is automatically excluded so Let's Encrypt
//! can still issue certificates.
//!
//! Generate a password hash with:
//!
//! ```sh
//! docker run --rm caddy:2-alpine caddy hash-password
//! ```
//!
//! ```rust,no_run
//! use catapulta::{
//!     App, Caddy, DigitalOcean, DockerSaveLoad, Ovh, Pipeline,
//! };
//!
//! fn main() -> anyhow::Result<()> {
//!     let app = App::new("internal-tool")
//!         .dockerfile("Dockerfile")
//!         .env("SERVER_HOST", "0.0.0.0")
//!         .env("SERVER_PORT", "8080")
//!         .healthcheck("curl -f http://localhost:8080/health")
//!         .expose(8080);
//!
//!     let caddy = Caddy::new()
//!         .basic_auth(
//!             "admin",
//!             "$2a$14$YOUR_BCRYPT_HASH_HERE",
//!         )
//!         .reverse_proxy("internal-tool:8080")
//!         .gzip()
//!         .security_headers();
//!
//!     let pipeline = Pipeline::new(app, caddy)
//!         .provision(DigitalOcean::new().region("nyc1"))
//!         .dns(Ovh::new("tool.example.com"))
//!         .deploy(DockerSaveLoad::new());
//!
//!     pipeline.run()?;
//!     Ok(())
//! }
//! ```
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
