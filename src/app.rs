use std::fmt;

/// A resolved upstream address: container name + port.
///
/// Produced by [`App::upstream`] and [`App::upstream_port`] so
/// the compiler catches mismatches between app names/ports and
/// Caddy configuration.
///
/// # Example
///
/// ```
/// use catapulta::App;
///
/// let app = App::new("api").expose(8000);
/// let upstream = app.upstream();
///
/// assert_eq!(upstream.to_string(), "api:8000");
/// ```
#[derive(Debug, Clone)]
pub struct Upstream {
    pub name: String,
    pub port: u16,
}

impl fmt::Display for Upstream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.port)
    }
}

/// Defines the application container: image, environment,
/// volumes, health checks, and exposed ports.
///
/// # Example
///
/// ```
/// use catapulta::App;
///
/// let app = App::new("my-service")
///     .dockerfile("Dockerfile")
///     .env("SERVER_HOST", "0.0.0.0")
///     .env("SERVER_PORT", "3000")
///     .volume("app-data", "/app/data")
///     .healthcheck("curl -f http://localhost:3000/")
///     .expose(3000)
///     .port(4222, 4222);
///
/// assert_eq!(app.name, "my-service");
/// assert_eq!(app.expose, vec![3000]);
/// assert_eq!(app.ports, vec![(4222, 4222)]);
/// ```
#[derive(Debug, Clone)]
pub struct App {
    pub name: String,
    pub dockerfile: String,
    pub platform: String,
    pub build_args: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
    pub env_file: Option<String>,
    pub volumes: Vec<(String, String)>,
    pub expose: Vec<u16>,
    pub ports: Vec<(u16, u16)>,
    pub healthcheck: Option<String>,
    pub context: Option<String>,
}

impl App {
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            dockerfile: "Dockerfile".to_string(),
            platform: "linux/amd64".to_string(),
            build_args: Vec::new(),
            env: Vec::new(),
            env_file: None,
            volumes: Vec::new(),
            expose: Vec::new(),
            ports: Vec::new(),
            healthcheck: None,
            context: None,
        }
    }

    #[must_use]
    pub fn dockerfile(mut self, path: &str) -> Self {
        self.dockerfile = path.to_string();
        self
    }

    #[must_use]
    pub fn platform(mut self, platform: &str) -> Self {
        self.platform = platform.to_string();
        self
    }

    #[must_use]
    pub fn build_arg(mut self, key: &str, value: &str) -> Self {
        self.build_args.push((key.to_string(), value.to_string()));
        self
    }

    #[must_use]
    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    #[must_use]
    pub fn env_file(mut self, path: &str) -> Self {
        self.env_file = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn volume(mut self, name: &str, mount: &str) -> Self {
        self.volumes.push((name.to_string(), mount.to_string()));
        self
    }

    #[must_use]
    pub fn expose(mut self, port: u16) -> Self {
        self.expose.push(port);
        self
    }

    /// Map a host port to a container port.
    ///
    /// This renders as `"host:container"` under the `ports` key in
    /// docker-compose, making the port accessible from outside the
    /// Docker network.
    #[must_use]
    pub fn port(mut self, host: u16, container: u16) -> Self {
        self.ports.push((host, container));
        self
    }

    #[must_use]
    pub fn context(mut self, path: &str) -> Self {
        self.context = Some(path.to_string());
        self
    }

    #[must_use]
    pub fn healthcheck(mut self, cmd: &str) -> Self {
        self.healthcheck = Some(cmd.to_string());
        self
    }

    /// Return an [`Upstream`] using the first exposed port.
    ///
    /// # Panics
    ///
    /// Panics if no ports have been exposed via [`App::expose`].
    #[must_use]
    pub fn upstream(&self) -> Upstream {
        let port = self
            .expose
            .first()
            .expect("upstream() requires at least one exposed port");
        Upstream {
            name: self.name.clone(),
            port: *port,
        }
    }

    /// Return an [`Upstream`] for a specific port.
    ///
    /// # Panics
    ///
    /// Panics if `port` is not in the list of exposed ports.
    #[must_use]
    pub fn upstream_port(&self, port: u16) -> Upstream {
        assert!(
            self.expose.contains(&port),
            "port {port} is not exposed on app '{}'",
            self.name,
        );
        Upstream {
            name: self.name.clone(),
            port,
        }
    }
}
