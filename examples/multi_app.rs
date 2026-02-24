//! Multi-app deployment pipeline example.
//!
//! Deploys an API backend and a web frontend behind a single
//! Caddy reverse proxy with path-based routing.
//!
//! ```sh
//! # Provision a new server
//! cargo xtask provision my-project --domain project.example.com
//!
//! # Deploy both services
//! cargo xtask deploy project.example.com
//!
//! # Preview generated config without deploying
//! cargo xtask deploy project.example.com --dry-run
//!
//! # Tear everything down
//! cargo xtask destroy my-project --domain project.example.com
//! ```

use catapulta::{App, Caddy, DigitalOcean, DockerSaveLoad, Ovh, Pipeline};

fn main() -> anyhow::Result<()> {
    let api = App::new("api")
        .dockerfile("api/Dockerfile")
        .env("DATABASE_URL", "sqlite:/app/data/api.db")
        .env_file("deploy/.env.api")
        .volume("api-data", "/app/data")
        .healthcheck("curl -f http://localhost:8000/health")
        .expose(8000);

    let web = App::new("web")
        .dockerfile("web/Dockerfile")
        .env("API_URL", "http://api:8000")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    // Caddy routes requests by path:
    //   /api/*  ->  api:8000
    //   /*      ->  web:3000   (catch-all)
    let caddy = Caddy::new()
        .route("/api/*", "api:8000")
        .route("", "web:3000")
        .gzip()
        .security_headers();

    let pipeline = Pipeline::multi(vec![api, web], caddy)
        .provision(DigitalOcean::new().size("s-1vcpu-2gb"))
        .dns(Ovh::new("project.example.com"))
        .deploy(DockerSaveLoad::new());

    pipeline.run()?;
    Ok(())
}
