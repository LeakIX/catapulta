use catapulta::DigitalOcean;
use catapulta::provision::digitalocean::remove_ssh_host_entry;

#[test]
fn defaults() {
    let do_ = DigitalOcean::new();

    assert_eq!(do_.size, "s-1vcpu-1gb");
    assert_eq!(do_.region, "fra1");
    assert_eq!(do_.image, "ubuntu-24-04-x64");
}

#[test]
fn builder_chain() {
    let do_ = DigitalOcean::new()
        .size("s-2vcpu-4gb")
        .region("nyc1")
        .image("ubuntu-22-04-x64");

    assert_eq!(do_.size, "s-2vcpu-4gb");
    assert_eq!(do_.region, "nyc1");
    assert_eq!(do_.image, "ubuntu-22-04-x64");
}

#[test]
fn remove_single_host_entry() {
    let config = "\
Host myserver
    HostName 1.2.3.4
    User root
    IdentityFile ~/.ssh/key

Host other
    HostName 5.6.7.8
    User deploy";

    let result = remove_ssh_host_entry(config, "myserver");

    assert!(!result.contains("Host myserver"));
    assert!(!result.contains("1.2.3.4"));
    assert!(result.contains("Host other"));
    assert!(result.contains("5.6.7.8"));
}

#[test]
fn remove_last_host_entry() {
    let config = "\
Host first
    HostName 1.1.1.1

Host target
    HostName 2.2.2.2
    User root";

    let result = remove_ssh_host_entry(config, "target");

    assert!(result.contains("Host first"));
    assert!(result.contains("1.1.1.1"));
    assert!(!result.contains("Host target"));
    assert!(!result.contains("2.2.2.2"));
}

#[test]
fn remove_nonexistent_host() {
    let config = "\
Host existing
    HostName 1.1.1.1
    User root";

    let result = remove_ssh_host_entry(config, "missing");

    assert!(result.contains("Host existing"));
    assert!(result.contains("1.1.1.1"));
}

#[test]
fn remove_from_empty_config() {
    let result = remove_ssh_host_entry("", "any");
    assert_eq!(result, "");
}

#[test]
fn remove_only_host_entry() {
    let config = "\
Host only
    HostName 1.1.1.1
    User root
    IdentityFile ~/.ssh/key";

    let result = remove_ssh_host_entry(config, "only");

    assert!(!result.contains("Host only"));
    assert!(!result.contains("1.1.1.1"));
}

#[test]
fn remove_collapses_triple_blank_lines() {
    let config = "\
Host a
    HostName 1.1.1.1



Host target
    HostName 2.2.2.2



Host b
    HostName 3.3.3.3";

    let result = remove_ssh_host_entry(config, "target");

    assert!(!result.contains("\n\n\n"));
    assert!(result.contains("Host a"));
    assert!(result.contains("Host b"));
}
