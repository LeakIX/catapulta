//! Integration test: clone a public Git repo and build its
//! Docker image using `.source()`.
//!
//! Requires Docker and Git. Skipped in normal `cargo test` runs
//! unless the `integration` feature is enabled.

#![cfg(feature = "integration")]

use catapulta::App;
use catapulta::DockerSaveLoad;
use catapulta::deploy::Deployer;

#[test]
fn build_from_git_source() {
    let app = App::new("ms-waitlist-notifier-test").source(
        "https://github.com/dannywillems/ms-waitlist-notifier.git",
        "main",
    );

    let deployer = DockerSaveLoad::new();
    deployer.build_image(&app).expect("docker build failed");
}
