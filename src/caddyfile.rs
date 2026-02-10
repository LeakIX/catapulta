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
