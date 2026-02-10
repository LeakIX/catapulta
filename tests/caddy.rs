use catapulta::Caddy;

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
