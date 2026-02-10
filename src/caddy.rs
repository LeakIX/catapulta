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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let caddy = Caddy::new();

        assert!(caddy.basic_auth.is_none());
        assert!(caddy.reverse_proxy.is_none());
        assert!(!caddy.gzip);
        assert!(!caddy.security_headers);
        assert!(caddy.extra_directives.is_empty());
    }

    #[test]
    fn builder_chain() {
        let caddy = Caddy::new()
            .basic_auth("admin", "$2a$14$hash")
            .reverse_proxy("app:3000")
            .gzip()
            .security_headers()
            .directive("log")
            .directive("tls internal");

        assert_eq!(
            caddy.basic_auth,
            Some(("admin".into(), "$2a$14$hash".into()))
        );
        assert_eq!(caddy.reverse_proxy.as_deref(), Some("app:3000"));
        assert!(caddy.gzip);
        assert!(caddy.security_headers);
        assert_eq!(caddy.extra_directives, vec!["log", "tls internal"]);
    }

    #[test]
    fn basic_auth_overrides() {
        let caddy = Caddy::new()
            .basic_auth("first", "hash1")
            .basic_auth("second", "hash2");

        assert_eq!(caddy.basic_auth, Some(("second".into(), "hash2".into())));
    }

    #[test]
    fn reverse_proxy_overrides() {
        let caddy = Caddy::new()
            .reverse_proxy("app:3000")
            .reverse_proxy("app:8080");

        assert_eq!(caddy.reverse_proxy.as_deref(), Some("app:8080"));
    }
}
