use catapulta::compose;
use catapulta::{App, Caddy};
use docker_compose_types::Compose;

#[test]
fn generates_valid_compose() {
    let app = App::new("myapp")
        .env("SERVER_HOST", "0.0.0.0")
        .env("SERVER_PORT", "3000")
        .volume("app-data", "/app/data")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .gzip()
        .security_headers();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("services:"));
    assert!(result.contains("caddy:"));
    assert!(result.contains("image: myapp:latest"));
    assert!(result.contains("app-data:/app/data"));
    assert!(result.contains("caddy-data:"));
    assert!(result.contains("myapp-network:"));
}

#[test]
fn no_caddy_service_without_reverse_proxy() {
    let app = App::new("standalone").expose(8080);
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("services:"));
    assert!(!result.contains("  caddy:"));
    assert!(result.contains("standalone:"));
    assert!(!result.contains("caddy-data:"));
    assert!(!result.contains("caddy-config:"));
}

#[test]
fn env_file_in_compose() {
    let app = App::new("myapp").env_file(".env").env("EXTRA", "val");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("env_file:"));
    assert!(result.contains(".env"));
    assert!(result.contains("environment:"));
    assert!(result.contains("EXTRA=val"));
}

#[test]
fn env_file_uses_filename_only() {
    let app = App::new("myapp").env_file("deploy/vps/.env");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains(".env"));
    assert!(!result.contains("deploy/vps/.env"));
}

#[test]
fn multiple_ports() {
    let app = App::new("multi").expose(3000).expose(8080).expose(9090);
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("expose:"));
    assert!(result.contains("3000"));
    assert!(result.contains("8080"));
    assert!(result.contains("9090"));
}

#[test]
fn no_caddy_volumes_when_no_caddy() {
    let app = App::new("novol");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(!result.contains("caddy-data"));
    assert!(!result.contains("caddy-config"));
}

#[test]
fn healthcheck_in_compose() {
    let app = App::new("hc").healthcheck("curl -f http://localhost:3000/");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("healthcheck:"));
    assert!(result.contains("interval: 30s"));
    assert!(result.contains("timeout: 10s"));
    assert!(result.contains("retries: 3"));
    assert!(result.contains("start_period: 10s"));
}

#[test]
fn no_healthcheck_when_unset() {
    let app = App::new("nohc");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(!result.contains("healthcheck:"));
}

#[test]
fn multiple_volumes() {
    let app = App::new("vols")
        .volume("data", "/app/data")
        .volume("config", "/app/config")
        .volume("logs", "/app/logs");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("data:/app/data"));
    assert!(result.contains("config:/app/config"));
    assert!(result.contains("logs:/app/logs"));
    assert!(result.contains("data:"));
    assert!(result.contains("config:"));
    assert!(result.contains("logs:"));
}

#[test]
fn caddy_depends_on_app() {
    let app = App::new("webapp").expose(3000);
    let caddy = Caddy::new().reverse_proxy(app.upstream());

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("depends_on:"));
    assert!(result.contains("webapp:"));
    assert!(result.contains("condition: service_healthy"));
}

#[test]
fn network_name_matches_app() {
    let app = App::new("my-service");
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("my-service-network:"));
    assert!(result.contains("driver: bridge"));
}

#[test]
fn round_trip_parse() {
    let app = App::new("roundtrip")
        .env("KEY", "value")
        .env_file(".env")
        .volume("data", "/app/data")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .gzip()
        .security_headers();

    let yaml = compose::render(&[app], &caddy);
    let parsed: Compose = serde_yaml::from_str(&yaml).expect("round-trip parse");

    assert!(parsed.services.0.contains_key("caddy"));
    assert!(parsed.services.0.contains_key("roundtrip"));
    assert!(parsed.volumes.0.contains_key("data"));
    assert!(parsed.volumes.0.contains_key("caddy-data"));
    assert!(parsed.networks.0.contains_key("roundtrip-network"));
}

// --- Host port mapping tests ---

#[test]
fn port_mapping_in_compose() {
    let app = App::new("nats").expose(4222).port(4222, 4222);
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("ports:"));
    assert!(result.contains("4222:4222"));
    // expose is still present separately
    assert!(result.contains("expose:"));
}

#[test]
fn multiple_port_mappings() {
    let app = App::new("nats").port(4222, 4222).port(8222, 8222);
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("4222:4222"));
    assert!(result.contains("8222:8222"));
}

#[test]
fn different_host_and_container_ports() {
    let app = App::new("db").port(15432, 5432);
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    assert!(result.contains("15432:5432"));
}

#[test]
fn no_ports_when_unset() {
    let app = App::new("internal").expose(3000);
    let caddy = Caddy::new();

    let result = compose::render(&[app], &caddy);

    // Should not have a ports key for the app service
    // (Caddy would have ports, but there's no Caddy here)
    let parsed: Compose = serde_yaml::from_str(&result).expect("parse");
    let svc = parsed.services.0.get("internal").unwrap();
    let svc = svc.as_ref().unwrap();
    assert!(matches!(svc.ports, docker_compose_types::Ports::Short(ref v) if v.is_empty()));
}

#[test]
fn port_mapping_round_trip() {
    let app = App::new("nats")
        .expose(4222)
        .port(4222, 4222)
        .port(8222, 8222);
    let caddy = Caddy::new();

    let yaml = compose::render(&[app], &caddy);
    let parsed: Compose = serde_yaml::from_str(&yaml).expect("round-trip parse");

    let svc = parsed.services.0.get("nats").unwrap();
    let svc = svc.as_ref().unwrap();
    match &svc.ports {
        docker_compose_types::Ports::Short(v) => {
            assert_eq!(v.len(), 2);
            assert!(v.contains(&"4222:4222".to_string()));
            assert!(v.contains(&"8222:8222".to_string()));
        }
        _ => panic!("expected short ports format"),
    }
}

// --- Multi-app tests ---

#[test]
fn multi_app_compose() {
    let api = App::new("api")
        .healthcheck("curl -f http://localhost:8000/health")
        .expose(8000);
    let web = App::new("web")
        .healthcheck("curl -f http://localhost:3000/")
        .expose(3000);

    let caddy = Caddy::new()
        .route("/api/*", api.upstream())
        .route("", web.upstream());

    let result = compose::render(&[api, web], &caddy);

    // Both services present
    assert!(result.contains("image: api:latest"));
    assert!(result.contains("image: web:latest"));

    // Caddy present (routes count as upstreams)
    assert!(result.contains("caddy:"));
    assert!(result.contains("caddy-data:"));

    // Shared network named after first app
    assert!(result.contains("api-network:"));

    // Caddy depends on both apps
    assert!(result.contains("api:"));
    assert!(result.contains("web:"));
    assert!(result.contains("condition: service_healthy"));
}

#[test]
fn multi_app_shared_network() {
    let api = App::new("api").expose(8000);
    let web = App::new("web").expose(3000);

    let caddy = Caddy::new()
        .route("/api/*", api.upstream())
        .route("", web.upstream());

    let yaml = compose::render(&[api, web], &caddy);
    let parsed: Compose = serde_yaml::from_str(&yaml).expect("parse");

    // Single shared network
    assert_eq!(parsed.networks.0.len(), 1);
    assert!(parsed.networks.0.contains_key("api-network"));

    // All three services exist
    assert!(parsed.services.0.contains_key("caddy"));
    assert!(parsed.services.0.contains_key("api"));
    assert!(parsed.services.0.contains_key("web"));
}

#[test]
fn multi_app_volumes_from_all_apps() {
    let api = App::new("api").volume("api-data", "/data").expose(8000);
    let web = App::new("web").volume("web-assets", "/assets").expose(3000);

    let caddy = Caddy::new()
        .route("/api/*", api.upstream())
        .route("", web.upstream());

    let yaml = compose::render(&[api, web], &caddy);
    let parsed: Compose = serde_yaml::from_str(&yaml).expect("parse");

    assert!(parsed.volumes.0.contains_key("api-data"));
    assert!(parsed.volumes.0.contains_key("web-assets"));
    assert!(parsed.volumes.0.contains_key("caddy-data"));
    assert!(parsed.volumes.0.contains_key("caddy-config"));
}

#[test]
fn caddy_custom_volumes_in_service() {
    let app = App::new("spa").expose(3000);
    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .volume("./web-static", "/www:ro")
        .volume("caddy-certs", "/certs");

    let result = compose::render(&[app], &caddy);

    // Custom volumes appear in the caddy service
    assert!(result.contains("./web-static:/www:ro"));
    assert!(result.contains("caddy-certs:/certs"));
    // Hardcoded volumes still present
    assert!(result.contains("caddy-data:/data"));
    assert!(result.contains("caddy-config:/config"));
}

#[test]
fn caddy_named_volume_registered_at_top_level() {
    let app = App::new("spa").expose(3000);
    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .volume("caddy-certs", "/certs");

    let yaml = compose::render(&[app], &caddy);
    let parsed: Compose = serde_yaml::from_str(&yaml).expect("parse");

    assert!(parsed.volumes.0.contains_key("caddy-certs"));
}

#[test]
fn caddy_bind_mount_not_in_top_level_volumes() {
    let app = App::new("spa").expose(3000);
    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .volume("./web-static", "/www:ro")
        .volume("/host/path", "/container:ro");

    let yaml = compose::render(&[app], &caddy);
    let parsed: Compose = serde_yaml::from_str(&yaml).expect("parse");

    // Bind mounts should NOT be in top-level volumes
    assert!(!parsed.volumes.0.contains_key("./web-static"));
    assert!(!parsed.volumes.0.contains_key("/host/path"));
    // But the service-level mount should still exist
    assert!(yaml.contains("./web-static:/www:ro"));
    assert!(yaml.contains("/host/path:/container:ro"));
}
