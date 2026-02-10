use std::path::Path;

use docker_compose_types::{
    Compose, ComposeNetworks, ComposeVolume, DependsCondition, DependsOnOptions, Environment,
    Healthcheck, HealthcheckTest, Labels, MapOrEmpty, NetworkSettings, Networks, Ports, Service,
    Services, TopLevelVolumes, Volumes,
};
use indexmap::IndexMap;

use crate::app::App;
use crate::caddy::Caddy;

/// Render a complete `docker-compose.yml` from App and Caddy
/// configuration.
#[must_use]
pub fn render(app: &App, caddy: &Caddy) -> String {
    let mut services = IndexMap::new();

    if caddy.reverse_proxy.is_some() {
        services.insert("caddy".to_string(), Some(caddy_service(app)));
    }

    services.insert(app.name.clone(), Some(app_service(app)));

    let compose = Compose {
        services: Services(services),
        volumes: top_level_volumes(app, caddy),
        networks: network(app),
        ..Default::default()
    };

    serde_yaml::to_string(&compose).expect("failed to serialize compose")
}

fn caddy_service(app: &App) -> Service {
    let mut depends = IndexMap::new();
    depends.insert(app.name.clone(), DependsCondition::service_healthy());

    Service {
        image: Some("caddy:2-alpine".to_string()),
        container_name: Some(format!("{}-caddy", app.name)),
        restart: Some("unless-stopped".to_string()),
        ports: Ports::Short(vec!["80:80".to_string(), "443:443".to_string()]),
        volumes: vec![
            Volumes::Simple("./Caddyfile:/etc/caddy/Caddyfile:ro".to_string()),
            Volumes::Simple("caddy-data:/data".to_string()),
            Volumes::Simple("caddy-config:/config".to_string()),
        ],
        depends_on: DependsOnOptions::Conditional(depends),
        networks: Networks::Simple(vec![format!("{}-network", app.name)]),
        ..Default::default()
    }
}

fn app_service(app: &App) -> Service {
    let expose: Vec<String> = app.expose.iter().map(ToString::to_string).collect();

    let env_file = app.env_file.as_ref().map(|ef| {
        let name = Path::new(ef)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(ef);
        docker_compose_types::StringOrList::Simple(name.to_string())
    });

    let environment = if app.env.is_empty() {
        Environment::default()
    } else {
        Environment::List(app.env.iter().map(|(k, v)| format!("{k}={v}")).collect())
    };

    let volumes: Vec<Volumes> = app
        .volumes
        .iter()
        .map(|(name, mount)| Volumes::Simple(format!("{name}:{mount}")))
        .collect();

    let healthcheck = app.healthcheck.as_ref().map(|cmd| Healthcheck {
        test: Some(HealthcheckTest::Multiple(vec![
            "CMD".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            cmd.clone(),
        ])),
        interval: Some("30s".to_string()),
        timeout: Some("10s".to_string()),
        retries: 3,
        start_period: Some("10s".to_string()),
        ..Default::default()
    });

    Service {
        image: Some(format!("{}:latest", app.name)),
        container_name: Some(app.name.clone()),
        restart: Some("unless-stopped".to_string()),
        expose,
        env_file,
        environment,
        volumes,
        healthcheck,
        networks: Networks::Simple(vec![format!("{}-network", app.name)]),
        ..Default::default()
    }
}

fn local_volume() -> ComposeVolume {
    ComposeVolume {
        driver: Some("local".to_string()),
        driver_opts: IndexMap::new(),
        external: None,
        labels: Labels::default(),
        name: None,
    }
}

fn top_level_volumes(app: &App, caddy: &Caddy) -> TopLevelVolumes {
    let mut vols = IndexMap::new();

    for (name, _) in &app.volumes {
        vols.insert(name.clone(), MapOrEmpty::Map(local_volume()));
    }

    if caddy.reverse_proxy.is_some() {
        let local = MapOrEmpty::Map(local_volume());
        vols.insert("caddy-data".to_string(), local.clone());
        vols.insert("caddy-config".to_string(), local);
    }

    TopLevelVolumes(vols)
}

fn network(app: &App) -> ComposeNetworks {
    let mut nets = IndexMap::new();
    nets.insert(
        format!("{}-network", app.name),
        MapOrEmpty::Map(NetworkSettings {
            driver: Some("bridge".to_string()),
            ..Default::default()
        }),
    );
    ComposeNetworks(nets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caddy::Caddy;

    #[test]
    fn generates_valid_compose() {
        let app = App::new("myapp")
            .env("SERVER_HOST", "0.0.0.0")
            .env("SERVER_PORT", "3000")
            .volume("app-data", "/app/data")
            .healthcheck("curl -f http://localhost:3000/")
            .expose(3000);

        let caddy = Caddy::new()
            .reverse_proxy("myapp:3000")
            .gzip()
            .security_headers();

        let result = render(&app, &caddy);

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

        let result = render(&app, &caddy);

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

        let result = render(&app, &caddy);

        assert!(result.contains("env_file:"));
        assert!(result.contains(".env"));
        assert!(result.contains("environment:"));
        assert!(result.contains("EXTRA=val"));
    }

    #[test]
    fn env_file_uses_filename_only() {
        let app = App::new("myapp").env_file("deploy/vps/.env");
        let caddy = Caddy::new();

        let result = render(&app, &caddy);

        assert!(result.contains(".env"));
        assert!(!result.contains("deploy/vps/.env"));
    }

    #[test]
    fn multiple_ports() {
        let app = App::new("multi").expose(3000).expose(8080).expose(9090);
        let caddy = Caddy::new();

        let result = render(&app, &caddy);

        assert!(result.contains("expose:"));
        assert!(result.contains("3000"));
        assert!(result.contains("8080"));
        assert!(result.contains("9090"));
    }

    #[test]
    fn no_caddy_volumes_when_no_caddy() {
        let app = App::new("novol");
        let caddy = Caddy::new();

        let result = render(&app, &caddy);

        assert!(!result.contains("caddy-data"));
        assert!(!result.contains("caddy-config"));
    }

    #[test]
    fn healthcheck_in_compose() {
        let app = App::new("hc").healthcheck("curl -f http://localhost:3000/");
        let caddy = Caddy::new();

        let result = render(&app, &caddy);

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

        let result = render(&app, &caddy);

        assert!(!result.contains("healthcheck:"));
    }

    #[test]
    fn multiple_volumes() {
        let app = App::new("vols")
            .volume("data", "/app/data")
            .volume("config", "/app/config")
            .volume("logs", "/app/logs");
        let caddy = Caddy::new();

        let result = render(&app, &caddy);

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
        let caddy = Caddy::new().reverse_proxy("webapp:3000");

        let result = render(&app, &caddy);

        assert!(result.contains("depends_on:"));
        assert!(result.contains("webapp:"));
        assert!(result.contains("condition: service_healthy"));
    }

    #[test]
    fn network_name_matches_app() {
        let app = App::new("my-service");
        let caddy = Caddy::new();

        let result = render(&app, &caddy);

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
            .reverse_proxy("roundtrip:3000")
            .gzip()
            .security_headers();

        let yaml = render(&app, &caddy);
        let parsed: Compose = serde_yaml::from_str(&yaml).expect("round-trip parse");

        assert!(parsed.services.0.contains_key("caddy"));
        assert!(parsed.services.0.contains_key("roundtrip"));
        assert!(parsed.volumes.0.contains_key("data"));
        assert!(parsed.volumes.0.contains_key("caddy-data"));
        assert!(parsed.networks.0.contains_key("roundtrip-network"));
    }
}
