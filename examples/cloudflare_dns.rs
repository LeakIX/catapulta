//! Deployment using Cloudflare DNS instead of OVH.
//!
//! Requires `CF_API_TOKEN` environment variable set with a token
//! that has `Zone > DNS > Edit` permissions.

use catapulta::{App, Caddy, Cloudflare, DigitalOcean, DockerSaveLoad, Pipeline};

fn main() -> anyhow::Result<()> {
    let app = App::new("my-service")
        .dockerfile("Dockerfile")
        .env("PORT", "3000")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new().reverse_proxy("my-service:3000").gzip();

    let pipeline = Pipeline::new(app, caddy)
        .provision(DigitalOcean::new())
        .dns(Cloudflare::new("service.example.com"))
        .deploy(DockerSaveLoad::new());

    pipeline.run()?;
    Ok(())
}
