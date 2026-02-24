//! Libvirt/KVM deployment pipeline example.
//!
//! Demonstrates provisioning a VM on a local hypervisor using
//! libvirt, with bridged networking and a single Docker
//! container behind Caddy.
//!
//! Prerequisites:
//!
//! - A Linux machine with KVM/libvirt installed and reachable
//!   via SSH
//! - A bridge interface (e.g. `br0`) configured on the
//!   hypervisor
//! - An SSH key pair at `~/.ssh/id_homelab`
//!
//! ```sh
//! # Provision a new VM
//! cargo xtask provision my-app --domain app.homelab.local
//!
//! # Deploy the application
//! cargo xtask deploy app.homelab.local
//!
//! # Tear everything down
//! cargo xtask destroy my-app --domain app.homelab.local
//! ```

use catapulta::{App, Caddy, DockerSaveLoad, Libvirt, NetworkMode, Pipeline};

fn main() -> anyhow::Result<()> {
    let app = App::new("my-app")
        .dockerfile("Dockerfile")
        .env("SERVER_HOST", "0.0.0.0")
        .env("SERVER_PORT", "3000")
        .env("DATABASE_URL", "sqlite:/app/data/app.db")
        .volume("app-data", "/app/data")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new()
        .reverse_proxy("my-app:3000")
        .gzip()
        .security_headers();

    let pipeline = Pipeline::new(app, caddy)
        .provision(
            Libvirt::new("hypervisor.local", "~/.ssh/id_homelab")
                .network(NetworkMode::Bridged("br0".into()))
                .vcpus(2)
                .memory_mib(2048)
                .disk_gib(20),
        )
        .deploy(DockerSaveLoad::new());

    pipeline.run()?;
    Ok(())
}
