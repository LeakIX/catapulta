/// Configuration for the Caddy reverse proxy container.
///
/// # Example
///
/// ```
/// use catapulta::{App, Caddy};
///
/// let app = App::new("my-service").expose(3000);
///
/// let caddy = Caddy::new()
///     .reverse_proxy(app.upstream())
///     .volume("./web-static", "/www:ro")
///     .gzip()
///     .security_headers();
///
/// assert!(caddy.gzip);
/// assert!(caddy.security_headers);
/// assert_eq!(caddy.volumes.len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Caddy {
    pub basic_auth: Option<(String, String)>,
    pub reverse_proxy: Option<String>,
    /// Path-based routes for multi-service setups.
    /// Each entry is `(path_pattern, upstream)`.
    /// When non-empty, these are rendered as Caddy `handle`
    /// blocks instead of a single `reverse_proxy`.
    pub routes: Vec<(String, String)>,
    pub gzip: bool,
    pub security_headers: bool,
    pub extra_directives: Vec<String>,
    /// Custom volumes to mount into the Caddy container.
    /// Each entry is `(host_path_or_name, container_path)`.
    pub volumes: Vec<(String, String)>,
}

impl Caddy {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn basic_auth(mut self, user: &str, password_hash: &str) -> Self {
        self.basic_auth = Some((user.to_string(), password_hash.to_string()));
        self
    }

    #[must_use]
    pub fn reverse_proxy(mut self, upstream: impl Into<String>) -> Self {
        self.reverse_proxy = Some(upstream.into());
        self
    }

    #[must_use]
    pub const fn gzip(mut self) -> Self {
        self.gzip = true;
        self
    }

    #[must_use]
    pub const fn security_headers(mut self) -> Self {
        self.security_headers = true;
        self
    }

    /// Add a path-based route rendered as a Caddy `handle` block.
    ///
    /// Use `/*` suffix for prefix matching. The last route
    /// without a path matcher becomes the catch-all `handle`.
    #[must_use]
    pub fn route(mut self, path: &str, upstream: impl Into<String>) -> Self {
        self.routes.push((path.to_string(), upstream.into()));
        self
    }

    /// Returns true when Caddy should be included in the
    /// compose stack (has a `reverse_proxy` or routes).
    #[must_use]
    pub fn has_upstreams(&self) -> bool {
        self.reverse_proxy.is_some() || !self.routes.is_empty()
    }

    #[must_use]
    pub fn directive(mut self, raw: &str) -> Self {
        self.extra_directives.push(raw.to_string());
        self
    }

    /// Mount a volume into the Caddy container.
    ///
    /// Paths starting with `./` or `/` are treated as bind mounts
    /// and will not be registered as top-level named volumes.
    #[must_use]
    pub fn volume(mut self, host: &str, container: &str) -> Self {
        self.volumes.push((host.to_string(), container.to_string()));
        self
    }
}
