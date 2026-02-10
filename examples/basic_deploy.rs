//! Basic deployment pipeline example.
//!
//! Demonstrates provisioning a `DigitalOcean` droplet, setting up
//! OVH DNS, and deploying a Docker container with Caddy reverse
//! proxy.
//!
//! ```sh
//! # Provision a new server
//! cargo xtask provision my-app --domain app.example.com
//!
//! # Deploy the application
//! cargo xtask deploy app.example.com
//!
//! # Tear everything down
//! cargo xtask destroy my-app --domain app.example.com
//! ```

use catapulta::{App, Caddy, DigitalOcean, DockerSaveLoad, Ovh, Pipeline};

fn main() -> anyhow::Result<()> {
    let app = App::new("my-app")
        .dockerfile("Dockerfile")
        .env("SERVER_HOST", "0.0.0.0")
        .env("SERVER_PORT", "3000")
        .env("DATABASE_URL", "sqlite:/app/data/app.db")
        .env_file("deploy/.env")
        .volume("app-data", "/app/data")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new()
        .reverse_proxy("my-app:3000")
        .gzip()
        .security_headers();

    let pipeline = Pipeline::new(app, caddy)
        .provision(DigitalOcean::new().size("s-1vcpu-1gb"))
        .dns(Ovh::new("app.example.com"))
        .deploy(DockerSaveLoad::new());

    pipeline.run()?;
    Ok(())
}
