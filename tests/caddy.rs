use catapulta::{App, Caddy};

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
    let app = App::new("app").expose(3000);
    let caddy = Caddy::new()
        .basic_auth("admin", "$2a$14$hash")
        .reverse_proxy(app.upstream())
        .gzip()
        .security_headers()
        .directive("log")
        .directive("tls internal");

    assert_eq!(
        caddy.basic_auth,
        Some(("admin".into(), "$2a$14$hash".into()))
    );
    assert_eq!(
        caddy.reverse_proxy.map(|u| u.to_string()),
        Some("app:3000".to_string()),
    );
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
    let app = App::new("app").expose(3000).expose(8080);
    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .reverse_proxy(app.upstream_port(8080));

    assert_eq!(
        caddy.reverse_proxy.map(|u| u.to_string()),
        Some("app:8080".to_string()),
    );
}

#[test]
fn reverse_proxy_accepts_upstream() {
    let app = App::new("svc").expose(5000);

    let caddy = Caddy::new().reverse_proxy(app.upstream());

    assert_eq!(
        caddy.reverse_proxy.map(|u| u.to_string()),
        Some("svc:5000".to_string()),
    );
}

#[test]
fn route_accepts_upstream() {
    let api = App::new("api").expose(8000);
    let web = App::new("web").expose(3000);

    let caddy = Caddy::new()
        .route("/api/*", api.upstream())
        .route("", web.upstream());

    assert_eq!(caddy.routes.len(), 2);
    assert_eq!(caddy.routes[0].0, "/api/*");
    assert_eq!(caddy.routes[0].1.to_string(), "api:8000");
    assert_eq!(caddy.routes[1].0, "");
    assert_eq!(caddy.routes[1].1.to_string(), "web:3000");
}

#[test]
fn volume_builder() {
    let caddy = Caddy::new()
        .volume("./web-static", "/www:ro")
        .volume("caddy-certs", "/certs");

    assert_eq!(caddy.volumes.len(), 2);
    assert_eq!(caddy.volumes[0], ("./web-static".into(), "/www:ro".into()));
    assert_eq!(caddy.volumes[1], ("caddy-certs".into(), "/certs".into()));
}
