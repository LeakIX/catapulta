use caddyfile_rs::{Caddyfile, SiteBlock, format, parse, tokenize};
use catapulta::caddyfile;
use catapulta::{App, Caddy};

#[test]
fn full_caddyfile() {
    let app = App::new("app").expose(3000);
    let caddy = Caddy::new()
        .basic_auth("admin", "$2a$14$hash")
        .reverse_proxy(app.upstream())
        .gzip()
        .security_headers();

    let result = caddyfile::render(&caddy, "example.com");

    assert!(result.contains("example.com {"));
    assert!(result.contains("basic_auth @protected"));
    assert!(result.contains("admin $2a$14$hash"));
    assert!(result.contains("reverse_proxy app:3000"));
    assert!(result.contains("encode gzip"));
    assert!(result.contains("X-Frame-Options"));
}

#[test]
fn minimal_caddyfile() {
    let backend = App::new("backend").expose(8080);
    let caddy = Caddy::new().reverse_proxy(backend.upstream());

    let result = caddyfile::render(&caddy, "test.dev");

    assert!(result.contains("test.dev {"));
    assert!(result.contains("reverse_proxy backend:8080"));
    assert!(!result.contains("basic_auth"));
    assert!(!result.contains("encode gzip"));
}

#[test]
fn extra_directives() {
    let app = App::new("app").expose(3000);
    let caddy = Caddy::new()
        .reverse_proxy(app.upstream())
        .directive("log")
        .directive("tls internal");

    let result = caddyfile::render(&caddy, "local.dev");

    assert!(result.contains("\tlog"));
    assert!(result.contains("\ttls internal"));
}

#[test]
fn security_headers_only() {
    let caddy = Caddy::new().security_headers();

    let result = caddyfile::render(&caddy, "secure.dev");

    assert!(result.contains("X-Content-Type-Options \"nosniff\""));
    assert!(result.contains("X-Frame-Options \"DENY\""));
    assert!(result.contains("X-XSS-Protection"));
    assert!(result.contains("Referrer-Policy"));
    assert!(!result.contains("reverse_proxy"));
    assert!(!result.contains("basic_auth"));
    assert!(!result.contains("encode gzip"));
}

#[test]
fn gzip_only() {
    let caddy = Caddy::new().gzip();

    let result = caddyfile::render(&caddy, "fast.dev");

    assert!(result.contains("encode gzip"));
    assert!(!result.contains("header {"));
}

#[test]
fn basic_auth_excludes_acme() {
    let caddy = Caddy::new().basic_auth("admin", "$2a$14$hash");

    let result = caddyfile::render(&caddy, "auth.dev");

    assert!(result.contains("@protected"));
    assert!(result.contains("/.well-known/acme-challenge/*"));
}

#[test]
fn empty_caddy() {
    let caddy = Caddy::new();

    let result = caddyfile::render(&caddy, "empty.dev");

    assert!(result.contains("empty.dev {"));
    assert!(result.contains('}'));
    assert!(!result.contains("reverse_proxy"));
    assert!(!result.contains("basic_auth"));
    assert!(!result.contains("encode"));
    assert!(!result.contains("header"));
}

#[test]
fn parse_roundtrip() {
    let input = "\
example.com {
\treverse_proxy app:3000
\tencode gzip
\tlog
}
";
    let tokens = tokenize(input).expect("tokenize failed");
    let cf = parse(&tokens).expect("parse failed");
    let output = format(&cf);
    assert_eq!(output, input);
}

#[test]
fn builder_roundtrip() {
    let cf = Caddyfile::new().site(
        SiteBlock::new("example.com")
            .reverse_proxy("app:3000")
            .encode_gzip()
            .log(),
    );
    let formatted = format(&cf);
    let tokens = tokenize(&formatted).expect("tokenize failed");
    let parsed = parse(&tokens).expect("parse failed");

    assert_eq!(parsed.sites.len(), 1);
    assert_eq!(
        parsed.sites[0].directives.len(),
        cf.sites[0].directives.len()
    );
}

// --- Route-based (multi-app) caddyfile tests ---

#[test]
fn route_based_handle_blocks() {
    let api = App::new("api").expose(8000);
    let web = App::new("web").expose(3000);
    let caddy = Caddy::new()
        .route("/api/*", api.upstream())
        .route("", web.upstream());

    let result = caddyfile::render(&caddy, "example.com");

    assert!(result.contains("handle /api/*"));
    assert!(result.contains("reverse_proxy api:8000"));
    assert!(result.contains("reverse_proxy web:3000"));
    // Catch-all handle has no path matcher
    assert!(result.contains("\thandle {\n"));
}

#[test]
fn routes_override_reverse_proxy() {
    // When routes are set, reverse_proxy is ignored
    let ignored = App::new("ignored").expose(9999);
    let api = App::new("api").expose(8000);
    let web = App::new("web").expose(3000);
    let caddy = Caddy::new()
        .reverse_proxy(ignored.upstream())
        .route("/api/*", api.upstream())
        .route("", web.upstream());

    let result = caddyfile::render(&caddy, "example.com");

    assert!(!result.contains("ignored:9999"));
    assert!(result.contains("reverse_proxy api:8000"));
    assert!(result.contains("reverse_proxy web:3000"));
}

#[test]
fn routes_with_gzip_and_headers() {
    let api = App::new("api").expose(8000);
    let web = App::new("web").expose(3000);
    let caddy = Caddy::new()
        .route("/api/*", api.upstream())
        .route("", web.upstream())
        .gzip()
        .security_headers();

    let result = caddyfile::render(&caddy, "example.com");

    assert!(result.contains("handle /api/*"));
    assert!(result.contains("encode gzip"));
    assert!(result.contains("X-Frame-Options"));
}

#[test]
fn single_reverse_proxy_with_upstream() {
    let app = App::new("app").expose(3000);
    let caddy = Caddy::new().reverse_proxy(app.upstream()).gzip();

    let result = caddyfile::render(&caddy, "example.com");

    assert!(result.contains("reverse_proxy app:3000"));
    assert!(!result.contains("handle"));
}
