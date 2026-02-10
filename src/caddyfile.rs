use caddyfile_rs::{Caddyfile, Directive, SiteBlock, format};

use crate::caddy::Caddy;

/// Render a complete Caddyfile from the Caddy config.
#[must_use]
pub fn render(caddy: &Caddy, domain: &str) -> String {
    let mut site = SiteBlock::new(domain);

    if let Some((user, hash)) = &caddy.basic_auth {
        site = site.basic_auth(user, hash);
    }

    if let Some(upstream) = &caddy.reverse_proxy {
        site = site.reverse_proxy(upstream);
    }

    if caddy.gzip {
        site = site.encode_gzip();
    }

    if caddy.security_headers {
        site = site.security_headers();
    }

    for d in &caddy.extra_directives {
        site = site.directive(Directive::new(d));
    }

    let caddyfile = Caddyfile::new().site(site);
    format(&caddyfile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use caddyfile_rs::{parse, tokenize};

    #[test]
    fn full_caddyfile() {
        let caddy = Caddy::new()
            .basic_auth("admin", "$2a$14$hash")
            .reverse_proxy("app:3000")
            .gzip()
            .security_headers();

        let result = render(&caddy, "example.com");

        assert!(result.contains("example.com {"));
        assert!(result.contains("basic_auth @protected"));
        assert!(result.contains("admin $2a$14$hash"));
        assert!(result.contains("reverse_proxy app:3000"));
        assert!(result.contains("encode gzip"));
        assert!(result.contains("X-Frame-Options"));
    }

    #[test]
    fn minimal_caddyfile() {
        let caddy = Caddy::new().reverse_proxy("backend:8080");

        let result = render(&caddy, "test.dev");

        assert!(result.contains("test.dev {"));
        assert!(result.contains("reverse_proxy backend:8080"));
        assert!(!result.contains("basic_auth"));
        assert!(!result.contains("encode gzip"));
    }

    #[test]
    fn extra_directives() {
        let caddy = Caddy::new()
            .reverse_proxy("app:3000")
            .directive("log")
            .directive("tls internal");

        let result = render(&caddy, "local.dev");

        assert!(result.contains("\tlog"));
        assert!(result.contains("\ttls internal"));
    }

    #[test]
    fn security_headers_only() {
        let caddy = Caddy::new().security_headers();

        let result = render(&caddy, "secure.dev");

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

        let result = render(&caddy, "fast.dev");

        assert!(result.contains("encode gzip"));
        assert!(!result.contains("header {"));
    }

    #[test]
    fn basic_auth_excludes_acme() {
        let caddy = Caddy::new().basic_auth("admin", "$2a$14$hash");

        let result = render(&caddy, "auth.dev");

        assert!(result.contains("@protected"));
        assert!(result.contains("/.well-known/acme-challenge/*"));
    }

    #[test]
    fn empty_caddy() {
        let caddy = Caddy::new();

        let result = render(&caddy, "empty.dev");

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
}
