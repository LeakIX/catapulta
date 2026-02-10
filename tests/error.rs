use catapulta::error::DeployError;

#[test]
fn display_command_not_found() {
    let err = DeployError::CommandNotFound("docker".into());
    assert_eq!(err.to_string(), "command not found: docker");
}

#[test]
fn display_ssh_failed() {
    let err = DeployError::SshFailed("timeout".into());
    assert_eq!(err.to_string(), "SSH connection failed: timeout");
}

#[test]
fn display_prerequisite_missing() {
    let err = DeployError::PrerequisiteMissing("doctl".into());
    assert_eq!(err.to_string(), "prerequisite missing: doctl");
}

#[test]
fn display_server_not_found() {
    let err = DeployError::ServerNotFound("my-droplet".into());
    assert_eq!(err.to_string(), "server not found: my-droplet");
}

#[test]
fn display_dns_error() {
    let err = DeployError::DnsError("record failed".into());
    assert_eq!(err.to_string(), "DNS error: record failed");
}

#[test]
fn display_env_missing() {
    let err = DeployError::EnvMissing("API_KEY".into());
    assert_eq!(err.to_string(), "environment variable missing: API_KEY");
}

#[test]
fn display_file_not_found() {
    let err = DeployError::FileNotFound("config.toml".into());
    assert_eq!(err.to_string(), "file not found: config.toml");
}

#[test]
fn display_other() {
    let err = DeployError::Other("custom error".into());
    assert_eq!(err.to_string(), "custom error");
}

#[test]
fn from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err: DeployError = io_err.into();
    assert!(matches!(err, DeployError::Io(_)));
}

#[test]
fn from_json_error() {
    let json_err = serde_json::from_str::<Vec<u64>>("invalid").unwrap_err();
    let err: DeployError = json_err.into();
    assert!(matches!(err, DeployError::Json(_)));
}
