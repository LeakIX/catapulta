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
//! ## Home lab deployment with libvirt/KVM
//!
//! If you have a spare Linux machine (desktop, server, old
//! laptop), you can use it as a hypervisor to run VMs instead
//! of renting cloud servers. Catapulta's [`Libvirt`] provisioner
//! automates VM creation over SSH so your workflow stays the
//! same as with cloud providers.
//!
//! ### What is a hypervisor?
//!
//! A **hypervisor** is software that runs virtual machines (VMs).
//! **KVM** is the hypervisor built into the Linux kernel, and
//! **libvirt** is the management layer that provides tools like
//! `virsh` and `virt-install` to create and control VMs.
//! Together they let one physical machine run many isolated VMs,
//! each with its own OS, IP address, and resources.
//!
//! ### Minimum hardware
//!
//! Any `x86_64` Linux machine with:
//!
//! - 8 GB RAM (each VM uses 2 GB by default)
//! - 50 GB free disk space
//! - CPU with virtualization extensions (Intel VT-x or AMD-V,
//!   almost every CPU made after 2010 has this)
//!
//! Verify with: `grep -cE 'vmx|svm' /proc/cpuinfo` (should
//! print a number greater than 0).
//!
//! ### Setting up the hypervisor
//!
//! On the hypervisor machine (Ubuntu/Debian):
//!
//! ```sh
//! # Install KVM and libvirt
//! sudo apt update
//! sudo apt install -y qemu-kvm libvirt-daemon-system \
//!     virtinst genisoimage
//!
//! # Enable and start libvirtd
//! sudo systemctl enable --now libvirtd
//!
//! # Verify it works
//! virsh list --all
//! ```
//!
//! Make sure you can SSH into this machine from your development
//! workstation (the machine where you run `cargo xtask`).
//!
//! ### Networking: bridged vs NAT
//!
//! **NAT** (default) puts VMs behind a virtual router
//! (`virbr0`). VMs can reach the internet but are only
//! accessible from the hypervisor. Good for testing.
//!
//! **Bridged** connects VMs directly to your LAN. They get a
//! real IP on your network (e.g. `192.168.1.x`) and are
//! reachable from any device. Required if you want to expose
//! services or access VMs from other machines.
//!
//! To create a bridge on the hypervisor (netplan example):
//!
//! ```yaml
//! # /etc/netplan/01-bridge.yaml
//! network:
//!   version: 2
//!   ethernets:
//!     enp3s0:
//!       dhcp4: false
//!   bridges:
//!     br0:
//!       interfaces: [enp3s0]
//!       dhcp4: true
//! ```
//!
//! Then `sudo netplan apply`.
//!
//! ### Cloud images and cloud-init
//!
//! Instead of installing an OS from an ISO (like you would on a
//! physical machine), cloud images are pre-built disk images
//! that boot in seconds. **cloud-init** is the tool that
//! configures the image on first boot: it sets the hostname,
//! injects your SSH key, and runs any startup commands.
//!
//! Catapulta downloads the cloud image once, copies it for each
//! VM, and generates a small "seed ISO" containing the
//! cloud-init configuration. The VM reads this ISO on first
//! boot and configures itself automatically.
//!
//! ### SSH key setup
//!
//! Generate a key pair if you don't have one:
//!
//! ```sh
//! ssh-keygen -t ed25519 -f ~/.ssh/id_homelab
//! ```
//!
//! Pass the private key path to the Libvirt provisioner.
//! Catapulta reads the `.pub` sibling and injects it into the
//! VM via cloud-init.
//!
//! ### Complete example
//!
//! ```rust,no_run
//! use catapulta::{
//!     App, Caddy, DockerSaveLoad, Libvirt, NetworkMode,
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
//!     let caddy = Caddy::new()
//!         .reverse_proxy("my-service:3000")
//!         .gzip()
//!         .security_headers();
//!
//!     let pipeline = Pipeline::new(app, caddy)
//!         .provision(
//!             Libvirt::new("hypervisor.local", "~/.ssh/id_homelab")
//!                 .network(NetworkMode::Bridged("br0".into()))
//!                 .vcpus(2)
//!                 .memory_mib(2048)
//!                 .disk_gib(20),
//!         )
//!         .deploy(DockerSaveLoad::new());
//!
//!     pipeline.run()?;
//!     Ok(())
//! }
//! ```
//!
//! ### Troubleshooting
//!
//! **VM has no IP address:**
//! NAT VMs need the DHCP server on `virbr0` to be running.
//! Check with `virsh net-list`. If "default" is inactive, start
//! it: `virsh net-start default`. For bridged VMs, ensure the
//! bridge has a DHCP server on the LAN or configure a static IP
//! in cloud-init.
//!
//! **SSH connection timeout:**
//! The VM may still be booting. Catapulta retries automatically
//! (30 attempts, 10 seconds apart). If it still fails, SSH to
//! the hypervisor and check the VM console:
//! `virsh console <vm-name>`.
//!
//! **"virsh: command not found":**
//! libvirt is not installed on the hypervisor. See the setup
//! section above.
//!
//! **Permission denied on virsh:**
//! Add your hypervisor user to the libvirt group:
//! `sudo usermod -aG libvirt $USER`, then log out and back in.
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
pub use provision::libvirt::Libvirt;
pub use provision::libvirt::NetworkMode;
pub use provision::remove_ssh_host_entry;
