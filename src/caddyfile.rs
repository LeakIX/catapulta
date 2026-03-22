use caddyfile_rs::{Caddyfile, Directive, Matcher, SiteBlock, format};

use crate::caddy::Caddy;

/// Render a complete Caddyfile from the Caddy config.
#[must_use]
pub fn render(caddy: &Caddy, domain: &str) -> String {
    let mut site = SiteBlock::new(domain);

    if let Some((user, hash)) = &caddy.basic_auth {
        site = site.basic_auth(user, hash);
    }

    // Routes take precedence over single reverse_proxy
    if !caddy.routes.is_empty() {
        site = add_route_handles(site, &caddy.routes);
    } else if let Some(upstream) = &caddy.reverse_proxy {
        site = site.reverse_proxy(&upstream.to_string());
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

    if let Some(ref path) = caddy.maintenance_page {
        site = add_maintenance_page(site, path);
    }

    let caddyfile = Caddyfile::new().site(site);
    format(&caddyfile)
}

/// Add `handle_errors` block that serves a user-provided
/// maintenance page on 502, 503, and 504 errors.
fn add_maintenance_page(site: SiteBlock, path: &str) -> SiteBlock {
    use caddyfile_rs::Argument;

    let html =
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            panic!(
                "failed to read maintenance page at \
                 '{path}': {e}"
            )
        });

    // @deploying expression {err.status_code} in [502, 503, 504]
    let matcher_def = Directive::new("@deploying")
        .arg("expression")
        .arg("{err.status_code}")
        .arg("in")
        .arg("[502,")
        .arg("503,")
        .arg("504]");

    // header Content-Type "text/html; charset=utf-8"
    let header = Directive::new("header")
        .arg("Content-Type")
        .quoted_arg("text/html; charset=utf-8");

    // respond <<HTML ... HTML 200
    let respond = Directive::new("respond")
        .arg("200")
        .block(vec![Directive {
            name: String::new(),
            matcher: None,
            arguments: vec![Argument::Heredoc {
                marker: "HTML".to_string(),
                content: html,
            }],
            block: None,
        }]);

    // handle @deploying { ... }
    let handle = Directive::new("handle")
        .matcher(Matcher::Named("deploying".to_string()))
        .block(vec![header, respond]);

    // handle_errors { ... }
    let handle_errors = Directive::new("handle_errors")
        .block(vec![matcher_def, handle]);

    site.directive(handle_errors)
}

/// Build `handle` directives for path-based routing.
///
/// Routes with a path pattern get `handle <path> { ... }`.
/// A route with an empty path becomes a bare `handle { ... }`
/// (catch-all).
fn add_route_handles(mut site: SiteBlock, routes: &[(String, crate::app::Upstream)]) -> SiteBlock {
    for (path, upstream) in routes {
        let inner = vec![Directive::new("reverse_proxy").arg(&upstream.to_string())];
        let mut handle = Directive::new("handle");
        if !path.is_empty() {
            handle = handle.matcher(Matcher::Path(path.clone()));
        }
        handle = handle.block(inner);
        site = site.directive(handle);
    }
    site
}
