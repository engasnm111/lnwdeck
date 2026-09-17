//! Small bounded runner for provider CLIs.
//!
//! Built-in providers occasionally need to invoke an already-installed vendor
//! CLI to read quota. The runner resolves executables from `PATH`, captures only
//! stdout/stderr, and enforces a wall-clock timeout so refreshes cannot hang the
//! application indefinitely.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Captured result from one bounded CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Resolves an executable from the current `PATH` without invoking a shell.
pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    if name.trim().is_empty() {
        return None;
    }
    let candidate = PathBuf::from(name);
    if candidate.is_absolute() && candidate.is_file() {
        return Some(candidate);
    }
    let path = std::env::var_os("PATH")?;
    #[cfg(windows)]
    let extensions: Vec<String> = {
        let raw = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_string());
        let mut values = raw
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase())
            .collect::<Vec<_>>();
        values.push(String::new());
        values
    };
    #[cfg(not(windows))]
    let extensions = vec![String::new()];

    for dir in std::env::split_paths(&path) {
        for extension in &extensions {
            let file = if extension.is_empty() || Path::new(name).extension().is_some() {
                dir.join(name)
            } else {
                dir.join(format!("{name}{extension}"))
            };
            if file.is_file() {
                return Some(file);
            }
        }
    }
    None
}

#[cfg(windows)]
fn is_batch_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "cmd" | "bat"))
}

#[cfg(windows)]
fn safe_batch_argument(value: &str) -> bool {
    !value
        .chars()
        .any(|ch| matches!(ch, '&' | '|' | '<' | '>' | '^' | '%' | '!' | '\r' | '\n'))
}

fn build_command(executable: &Path, args: &[&str]) -> Result<Command, String> {
    #[cfg(windows)]
    if is_batch_file(executable) {
        let executable_text = executable.to_string_lossy();
        if !safe_batch_argument(&executable_text)
            || args.iter().any(|arg| !safe_batch_argument(arg))
        {
            return Err("UNSAFE_CLI_ARGUMENT".to_string());
        }
        let mut command = Command::new("cmd.exe");
        command
            .arg("/D")
            .arg("/S")
            .arg("/C")
            .arg("call")
            .arg(executable)
            .args(args);
        return Ok(command);
    }

    let mut command = Command::new(executable);
    command.args(args);
    Ok(command)
}

/// Runs an already-resolved provider CLI with a hard wall-clock timeout.
///
/// The caller should pass fixed, provider-owned arguments. The helper never
/// invokes a shell except for `.cmd`/`.bat` shims on Windows, where command
/// metacharacters are rejected before execution.
pub fn run_command(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<CommandOutput, String> {
    if !executable.is_file() {
        return Err("CLI_NOT_FOUND".to_string());
    }
    let mut command = build_command(executable, args)?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "CLI_START_FAILED".to_string())?;
    let started = Instant::now();
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                timed_out = true;
                let _ = child.kill();
                break;
            }
            Err(_) => {
                let _ = child.kill();
                return Err("CLI_WAIT_FAILED".to_string());
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "CLI_WAIT_FAILED".to_string())?;
    Ok(CommandOutput {
        status: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_binary_is_not_resolved() {
        assert!(resolve_binary("definitely-not-a-real-provider-cli-923847").is_none());
    }
}
