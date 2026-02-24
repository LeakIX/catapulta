use std::process::ExitStatus;

pub type DeployResult<T> = Result<T, DeployError>;

#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("command failed: {command}")]
    CommandFailed { command: String, status: ExitStatus },

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("SSH connection failed: {0}")]
    SshFailed(String),

    #[error("prerequisite missing: {0}")]
    PrerequisiteMissing(String),

    #[error("server not found: {0}")]
    ServerNotFound(String),

    #[error("DNS error: {0}")]
    DnsError(String),

    #[error("environment variable missing: {0}")]
    EnvMissing(String),

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error(
        "container '{0}' did not become healthy after {1} attempts"
    )]
    HealthcheckTimeout(String, u32),

    #[error("{0}")]
    Other(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
