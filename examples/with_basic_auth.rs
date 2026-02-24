//! Deployment with basic authentication.
//!
//! Protects the application behind HTTP basic auth using Caddy.
//! The ACME challenge path is automatically excluded so Let's
//! Encrypt can still issue certificates.
//!
//! Generate a password hash with:
//! ```sh
//! docker run --rm caddy:2-alpine caddy hash-password
//! ```

use catapulta::{App, Caddy, DigitalOcean, DockerSaveLoad, Ovh, Pipeline};

fn main() -> anyhow::Result<()> {
    let app = App::new("internal-tool")
        .dockerfile("Dockerfile")
        .env("SERVER_HOST", "0.0.0.0")
        .env("SERVER_PORT", "8080")
        .healthcheck("curl -f http://localhost:8080/health")
        .expose(8080);

    let caddy = Caddy::new()
        .basic_auth("admin", "$2a$14$YOUR_BCRYPT_HASH_HERE")
        .reverse_proxy(app.upstream())
        .gzip()
        .security_headers();

    let pipeline = Pipeline::new(app, caddy)
        .provision(DigitalOcean::new().region("nyc1"))
        .dns(Ovh::new("tool.example.com"))
        .deploy(DockerSaveLoad::new());

    pipeline.run()?;
    Ok(())
}
