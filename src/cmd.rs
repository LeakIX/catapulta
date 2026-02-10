use std::process::{Command, Output, Stdio};

use crate::error::{DeployError, DeployResult};

/// Run a command and capture its output. Fails if the command
/// returns a non-zero exit code.
pub fn run(program: &str, args: &[&str]) -> DeployResult<String> {
    let output = spawn(program, args)?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let command = format_command(program, args);
        eprintln!("stderr: {stderr}");
        Err(DeployError::CommandFailed {
            command,
            status: output.status,
        })
    }
}

/// Run a command with stdin/stdout/stderr inherited (interactive).
pub fn run_interactive(program: &str, args: &[&str]) -> DeployResult<()> {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DeployError::CommandNotFound(program.to_string())
            } else {
                DeployError::Io(e)
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(DeployError::CommandFailed {
            command: format_command(program, args),
            status,
        })
    }
}

/// Run a command that pipes its stdin from a byte slice.
pub fn run_with_stdin(program: &str, args: &[&str], stdin_data: &[u8]) -> DeployResult<String> {
    use std::io::Write;

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DeployError::CommandNotFound(program.to_string())
            } else {
                DeployError::Io(e)
            }
        })?;

    if let Some(stdin) = &mut child.stdin {
        stdin.write_all(stdin_data)?;
    }
    drop(child.stdin.take());

    let output = child.wait_with_output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        eprintln!("stderr: {stderr}");
        Err(DeployError::CommandFailed {
            command: format_command(program, args),
            status: output.status,
        })
    }
}

/// Run a shell pipeline (via `sh -c`).
pub fn run_pipeline(shell_cmd: &str) -> DeployResult<()> {
    run_interactive("sh", &["-c", shell_cmd])
}

/// Check if a command exists on PATH.
#[must_use]
pub fn command_exists(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn spawn(program: &str, args: &[&str]) -> DeployResult<Output> {
    Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                DeployError::CommandNotFound(program.to_string())
            } else {
                DeployError::Io(e)
            }
        })
}

fn format_command(program: &str, args: &[&str]) -> String {
    let mut parts = vec![program.to_string()];
    parts.extend(args.iter().map(|a| (*a).to_string()));
    parts.join(" ")
}
