use std::path::Path;

use docker_compose_types::{
    Compose, ComposeNetworks, ComposeVolume, DependsCondition, DependsOnOptions, Environment,
    Healthcheck, HealthcheckTest, Labels, MapOrEmpty, NetworkSettings, Networks, Ports, Service,
    Services, TopLevelVolumes, Volumes,
};
use indexmap::IndexMap;

use crate::app::App;
use crate::caddy::Caddy;

/// Render a complete `docker-compose.yml` from one or more Apps
/// and Caddy configuration.
#[must_use]
pub fn render(apps: &[App], caddy: &Caddy) -> String {
    assert!(!apps.is_empty(), "at least one app is required");

    let network_name = format!("{}-network", apps[0].name);
    let mut services = IndexMap::new();

    if caddy.has_upstreams() {
        services.insert(
            "caddy".to_string(),
            Some(caddy_service(apps, &network_name)),
        );
    }

    for app in apps {
        services.insert(app.name.clone(), Some(app_service(app, &network_name)));
    }

    let compose = Compose {
        services: Services(services),
        volumes: top_level_volumes(apps, caddy),
        networks: network(&network_name),
        ..Default::default()
    };

    serde_yaml::to_string(&compose).expect("failed to serialize compose")
}

fn caddy_service(apps: &[App], network_name: &str) -> Service {
    let mut depends = IndexMap::new();
    for app in apps {
        depends.insert(app.name.clone(), DependsCondition::service_healthy());
    }

    Service {
        image: Some("caddy:2-alpine".to_string()),
        container_name: Some(format!("{}-caddy", apps[0].name)),
        restart: Some("unless-stopped".to_string()),
        ports: Ports::Short(vec!["80:80".to_string(), "443:443".to_string()]),
        volumes: vec![
            Volumes::Simple("./Caddyfile:/etc/caddy/Caddyfile:ro".to_string()),
            Volumes::Simple("caddy-data:/data".to_string()),
            Volumes::Simple("caddy-config:/config".to_string()),
        ],
        depends_on: DependsOnOptions::Conditional(depends),
        networks: Networks::Simple(vec![network_name.to_string()]),
        ..Default::default()
    }
}

fn app_service(app: &App, network_name: &str) -> Service {
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
        networks: Networks::Simple(vec![network_name.to_string()]),
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

fn top_level_volumes(apps: &[App], caddy: &Caddy) -> TopLevelVolumes {
    let mut vols = IndexMap::new();

    for app in apps {
        for (name, _) in &app.volumes {
            vols.insert(name.clone(), MapOrEmpty::Map(local_volume()));
        }
    }

    if caddy.has_upstreams() {
        let local = MapOrEmpty::Map(local_volume());
        vols.insert("caddy-data".to_string(), local.clone());
        vols.insert("caddy-config".to_string(), local);
    }

    TopLevelVolumes(vols)
}

fn network(network_name: &str) -> ComposeNetworks {
    let mut nets = IndexMap::new();
    nets.insert(
        network_name.to_string(),
        MapOrEmpty::Map(NetworkSettings {
            driver: Some("bridge".to_string()),
            ..Default::default()
        }),
    );
    ComposeNetworks(nets)
}
