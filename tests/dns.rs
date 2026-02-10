use catapulta::dns::split_domain;

#[test]
fn split_fqdn() {
    let (zone, sub) = split_domain("app.example.com");
    assert_eq!(zone, "example.com");
    assert_eq!(sub, "app");
}

#[test]
fn split_bare_domain() {
    let (zone, sub) = split_domain("example.com");
    assert_eq!(zone, "example.com");
    assert_eq!(sub, "");
}

#[test]
fn split_deep_subdomain() {
    let (zone, sub) = split_domain("a.b.example.com");
    assert_eq!(zone, "example.com");
    assert_eq!(sub, "a.b");
}
