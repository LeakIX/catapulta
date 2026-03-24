//! Local deployment example.
//!
//! Runs the full Docker Compose stack on your machine with
//! Caddy using `tls internal` (self-signed certificates).
//!
//! ```sh
//! # Preview generated files without starting anything
//! cargo run --example local_deploy -- deploy-local myapp.local.dev --dry-run
//!
//! # Deploy locally
//! cargo run --example local_deploy -- deploy-local myapp.local.dev
//!
//! # Skip rebuild if images are already built
//! cargo run --example local_deploy -- deploy-local myapp.local.dev --skip-build
//!
//! # Check status
//! cargo run --example local_deploy -- local-status
//!
//! # Stop the stack
//! cargo run --example local_deploy -- local-down
//! ```

use catapulta::{App, Caddy, Pipeline};

fn main() -> anyhow::Result<()> {
    let app = App::new("my-app")
        .dockerfile("Dockerfile")
        .env("SERVER_HOST", "0.0.0.0")
        .env("SERVER_PORT", "3000")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .gzip()
        .security_headers();

    let pipeline = Pipeline::new(app, caddy);

    pipeline.run()?;
    Ok(())
}
