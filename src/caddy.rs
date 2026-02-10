/// Configuration for the Caddy reverse proxy container.
///
/// # Example
///
/// ```
/// use catapulta::Caddy;
///
/// let caddy = Caddy::new()
///     .reverse_proxy("my-service:3000")
///     .gzip()
///     .security_headers();
///
/// assert!(caddy.gzip);
/// assert!(caddy.security_headers);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Caddy {
    pub basic_auth: Option<(String, String)>,
    pub reverse_proxy: Option<String>,
    pub gzip: bool,
    pub security_headers: bool,
    pub extra_directives: Vec<String>,
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
    pub fn reverse_proxy(mut self, upstream: &str) -> Self {
        self.reverse_proxy = Some(upstream.to_string());
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

    #[must_use]
    pub fn directive(mut self, raw: &str) -> Self {
        self.extra_directives.push(raw.to_string());
        self
    }
}
