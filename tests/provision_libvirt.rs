use catapulta::provision::libvirt::{Libvirt, NetworkMode, parse_domifaddr};

#[test]
fn parse_domifaddr_nat_output() {
    let output = " Name       MAC address          Protocol     Address\n\
                   -------------------------------------------------------\n \
                   vnet0      52:54:00:ab:cd:ef    ipv4         192.168.122.45/24\n";

    let ip = parse_domifaddr(output);
    assert_eq!(ip, Some("192.168.122.45".to_string()));
}

#[test]
fn parse_domifaddr_bridged_arp_output() {
    let output = " Name       MAC address          Protocol     Address\n\
                   -------------------------------------------------------\n \
                   macvtap0   52:54:00:11:22:33    ipv4         10.0.0.50\n";

    let ip = parse_domifaddr(output);
    assert_eq!(ip, Some("10.0.0.50".to_string()));
}

#[test]
fn parse_domifaddr_empty_output() {
    let output = " Name       MAC address          Protocol     Address\n\
                   -------------------------------------------------------\n";

    let ip = parse_domifaddr(output);
    assert_eq!(ip, None);
}

#[test]
fn parse_domifaddr_completely_empty() {
    assert_eq!(parse_domifaddr(""), None);
}

#[test]
fn parse_domifaddr_no_ipv4_line() {
    let output = " Name       MAC address          Protocol     Address\n\
                   -------------------------------------------------------\n \
                   vnet0      52:54:00:ab:cd:ef    ipv6         fe80::1/64\n";

    let ip = parse_domifaddr(output);
    assert_eq!(ip, None);
}

#[test]
fn builder_defaults() {
    let lv = Libvirt::new("myhost", "/tmp/key");

    assert_eq!(lv.hypervisor_host, "myhost");
    assert_eq!(lv.hypervisor_user, "root");
    assert!(lv.hypervisor_key.is_none());
    assert_eq!(lv.vcpus, 2);
    assert_eq!(lv.memory_mib, 2048);
    assert_eq!(lv.disk_gib, 20);
    assert_eq!(lv.vm_ssh_key, "/tmp/key");
    assert_eq!(lv.os_variant, "ubuntu24.04");
    assert_eq!(lv.storage_dir, "/var/lib/libvirt/images");
    assert!(matches!(lv.network, NetworkMode::Nat));
}

#[test]
fn builder_chain() {
    let lv = Libvirt::new("myhost", "/tmp/key")
        .hypervisor_user("admin")
        .hypervisor_key("/tmp/hv_key")
        .vcpus(4)
        .memory_mib(4096)
        .disk_gib(50)
        .network(NetworkMode::Bridged("br0".into()))
        .storage_dir("/data/vms")
        .os_variant("debian12")
        .image_url("https://example.com/image.img");

    assert_eq!(lv.hypervisor_user, "admin");
    assert_eq!(lv.hypervisor_key, Some("/tmp/hv_key".to_string()));
    assert_eq!(lv.vcpus, 4);
    assert_eq!(lv.memory_mib, 4096);
    assert_eq!(lv.disk_gib, 50);
    assert_eq!(lv.storage_dir, "/data/vms");
    assert_eq!(lv.os_variant, "debian12");
    assert_eq!(lv.image_url, "https://example.com/image.img");
    assert!(matches!(
        lv.network,
        NetworkMode::Bridged(ref b) if b == "br0"
    ));
}

#[test]
fn network_mode_nat() {
    let mode = NetworkMode::Nat;
    assert!(matches!(mode, NetworkMode::Nat));
}

#[test]
fn network_mode_bridged() {
    let mode = NetworkMode::Bridged("virbr1".into());
    assert!(matches!(mode, NetworkMode::Bridged(ref b) if b == "virbr1"));
}
