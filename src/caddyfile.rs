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

    let caddyfile = Caddyfile::new().site(site);
    format(&caddyfile)
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
