use std::thread;
use std::time::Duration;

use crate::cmd;
use crate::error::{DeployError, DeployResult};

/// SSH session wrapper for executing commands and transferring
/// files to a remote host.
pub struct SshSession {
    host: String,
    user: String,
    key: Option<String>,
}

impl SshSession {
    #[must_use]
    pub fn new(host: &str, user: &str) -> Self {
        Self {
            host: host.to_string(),
            user: user.to_string(),
            key: None,
        }
    }

    #[must_use]
    pub fn with_key(mut self, key_path: &str) -> Self {
        self.key = Some(key_path.to_string());
        self
    }

    /// Execute a command on the remote host and capture output.
    pub fn exec(&self, command: &str) -> DeployResult<String> {
        let args = self.build_ssh_args(command);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        cmd::run("ssh", &refs)
    }

    /// Execute a command on the remote host interactively.
    pub fn exec_interactive(&self, command: &str) -> DeployResult<()> {
        let args = self.build_ssh_args(command);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        cmd::run_interactive("ssh", &refs)
    }

    /// Copy a local file to the remote host.
    pub fn scp_to(&self, local_path: &str, remote_path: &str) -> DeployResult<()> {
        let mut args = self.scp_base_args();
        let dest = format!("{}:{remote_path}", self.destination());
        args.push(local_path.to_string());
        args.push(dest);

        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        cmd::run_interactive("scp", &refs)
    }

    /// Write content to a remote file via stdin pipe.
    pub fn write_remote_file(&self, content: &str, remote_path: &str) -> DeployResult<()> {
        let command = format!("cat > {remote_path}");
        let args = self.build_ssh_args(&command);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        cmd::run_with_stdin("ssh", &refs, content.as_bytes())?;
        Ok(())
    }

    /// Wait for SSH to become available on the remote host.
    pub fn wait_for_ready(&self, max_attempts: u32, interval: Duration) -> DeployResult<()> {
        for attempt in 1..=max_attempts {
            eprint!(
                "Waiting for SSH \
                 ({attempt}/{max_attempts})... "
            );
            if self.exec("echo ok").is_ok() {
                eprintln!("connected");
                return Ok(());
            }
            eprintln!("retrying");
            thread::sleep(interval);
        }

        Err(DeployError::SshFailed(format!(
            "SSH not ready after {max_attempts} attempts \
             on {}",
            self.host
        )))
    }

    fn destination(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }

    fn build_ssh_args(&self, command: &str) -> Vec<String> {
        let mut args = self.ssh_base_args();
        args.push(self.destination());
        args.push(command.to_string());
        args
    }

    fn ssh_base_args(&self) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=accept-new".to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
        ];
        if let Some(key) = &self.key {
            args.push("-i".to_string());
            args.push(key.clone());
        }
        args
    }

    fn scp_base_args(&self) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=accept-new".to_string(),
        ];
        if let Some(key) = &self.key {
            args.push("-i".to_string());
            args.push(key.clone());
        }
        args
    }
}
