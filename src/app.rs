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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let app = App::new("myapp");

        assert_eq!(app.name, "myapp");
        assert_eq!(app.dockerfile, "Dockerfile");
        assert_eq!(app.platform, "linux/amd64");
        assert!(app.build_args.is_empty());
        assert!(app.env.is_empty());
        assert!(app.env_file.is_none());
        assert!(app.volumes.is_empty());
        assert!(app.expose.is_empty());
        assert!(app.healthcheck.is_none());
    }

    #[test]
    fn builder_chain() {
        let app = App::new("test")
            .dockerfile("deploy/Dockerfile")
            .platform("linux/arm64")
            .build_arg("RUST_VERSION", "1.93.0")
            .build_arg("NODE_VERSION", "24")
            .env("HOST", "0.0.0.0")
            .env("PORT", "3000")
            .env_file(".env")
            .volume("data", "/app/data")
            .volume("config", "/app/config")
            .expose(3000)
            .expose(8080)
            .healthcheck("curl -f http://localhost:3000/");

        assert_eq!(app.dockerfile, "deploy/Dockerfile");
        assert_eq!(app.platform, "linux/arm64");
        assert_eq!(
            app.build_args,
            vec![
                ("RUST_VERSION".into(), "1.93.0".into()),
                ("NODE_VERSION".into(), "24".into()),
            ]
        );
        assert_eq!(
            app.env,
            vec![
                ("HOST".into(), "0.0.0.0".into()),
                ("PORT".into(), "3000".into()),
            ]
        );
        assert_eq!(app.env_file.as_deref(), Some(".env"));
        assert_eq!(
            app.volumes,
            vec![
                ("data".into(), "/app/data".into()),
                ("config".into(), "/app/config".into()),
            ]
        );
        assert_eq!(app.expose, vec![3000, 8080]);
        assert_eq!(
            app.healthcheck.as_deref(),
            Some("curl -f http://localhost:3000/")
        );
    }

    #[test]
    fn env_file_overrides() {
        let app = App::new("x").env_file("first.env").env_file("second.env");

        assert_eq!(app.env_file.as_deref(), Some("second.env"));
    }
}
