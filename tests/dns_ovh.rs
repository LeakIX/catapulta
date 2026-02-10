use catapulta::dns::ovh::{Ovh, OvhCredentials, parse_ini_value};

#[test]
fn parse_ovh_conf() {
    let conf = "\
[default]
endpoint=ovh-eu

[ovh-eu]
application_key=abc123
application_secret=secret456
consumer_key=ck789
";
    assert_eq!(
        parse_ini_value(conf, "default", "endpoint"),
        Some("ovh-eu".into())
    );
    assert_eq!(
        parse_ini_value(conf, "ovh-eu", "application_key"),
        Some("abc123".into())
    );
    assert_eq!(
        parse_ini_value(conf, "ovh-eu", "consumer_key"),
        Some("ck789".into())
    );
}

#[test]
fn parse_ini_missing_key() {
    let conf = "\
[default]
endpoint=ovh-eu
";
    assert_eq!(parse_ini_value(conf, "default", "nonexistent"), None);
}

#[test]
fn parse_ini_missing_section() {
    let conf = "\
[default]
endpoint=ovh-eu
";
    assert_eq!(parse_ini_value(conf, "missing", "endpoint"), None);
}

#[test]
fn parse_ini_spaces_around_equals() {
    let conf = "\
[section]
key = value with spaces
";
    assert_eq!(
        parse_ini_value(conf, "section", "key"),
        Some("value with spaces".into())
    );
}

#[test]
fn parse_ini_stops_at_next_section() {
    let conf = "\
[first]
a=1

[second]
a=2
";
    assert_eq!(parse_ini_value(conf, "first", "a"), Some("1".into()));
    assert_eq!(parse_ini_value(conf, "second", "a"), Some("2".into()));
}

#[test]
fn parse_ini_key_not_in_section() {
    let conf = "\
[first]
x=1

[second]
y=2
";
    // Key y exists in second but not in first
    assert_eq!(parse_ini_value(conf, "first", "y"), None);
}

#[test]
fn parse_ini_empty_content() {
    assert_eq!(parse_ini_value("", "any", "key"), None);
}

#[test]
fn api_base_eu() {
    let creds = OvhCredentials {
        endpoint: "ovh-eu".into(),
        application_key: String::new(),
        application_secret: String::new(),
        consumer_key: String::new(),
    };
    assert_eq!(Ovh::api_base(&creds), "https://eu.api.ovh.com/1.0");
}

#[test]
fn api_base_us() {
    let creds = OvhCredentials {
        endpoint: "ovh-us".into(),
        application_key: String::new(),
        application_secret: String::new(),
        consumer_key: String::new(),
    };
    assert_eq!(Ovh::api_base(&creds), "https://api.us.ovhcloud.com/1.0");
}

#[test]
fn api_base_ca() {
    let creds = OvhCredentials {
        endpoint: "ovh-ca".into(),
        application_key: String::new(),
        application_secret: String::new(),
        consumer_key: String::new(),
    };
    assert_eq!(Ovh::api_base(&creds), "https://ca.api.ovh.com/1.0");
}

#[test]
fn api_base_unknown() {
    let creds = OvhCredentials {
        endpoint: "custom".into(),
        application_key: String::new(),
        application_secret: String::new(),
        consumer_key: String::new(),
    };
    assert_eq!(Ovh::api_base(&creds), "https://custom.api.ovh.com/1.0");
}

#[test]
fn ovh_domain() {
    let ovh = Ovh::new("app.example.com");
    assert_eq!(ovh.domain, "app.example.com");
}
