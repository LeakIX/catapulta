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
///     .expose(3000);
///
/// assert_eq!(app.name, "my-service");
/// assert_eq!(app.expose, vec![3000]);
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
    pub healthcheck: Option<String>,
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
            healthcheck: None,
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

    #[must_use]
    pub fn healthcheck(mut self, cmd: &str) -> Self {
        self.healthcheck = Some(cmd.to_string());
        self
    }
}
