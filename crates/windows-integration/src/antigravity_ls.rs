//! Discovery of the Antigravity IDE Language Server (LS).
//!
//! The Antigravity IDE keeps a local gRPC server (the Language Server) that
//! holds the user's Google session and can answer quota RPCs on their behalf.
//! lnwdeck never asks Google for credentials it cannot obtain passively: when
//! the IDE is running, the LS is reachable on a localhost port and the CSRF
//! token is printed on the LS command line. This module finds the LS process
//! and its gRPC endpoint so the gemini adapter can call
//! `RetrieveUserQuotaSummary` exactly like the IDE's own Settings -> Models
//! screen does.
//!
//! Everything here is read-only: no process is started, stopped or modified,
//! and no token or session material is persisted.

use std::process::Command;

/// Identifies an Antigravity IDE Language Server process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageServer {
    pub pid: u32,
    /// Localhost TCP ports owned by the LS, in netstat order. One of them is
    /// the plain-HTTP gRPC listener the quota RPC is served on; the caller
    /// probes them in order.
    pub ports: Vec<u16>,
    /// The CSRF token the IDE hands to the LS for gRPC calls.
    pub csrf_token: String,
}

/// Why discovery failed. Sanitized: never carries a path, token or pid that
/// could identify the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsDiscoveryError {
    /// No Antigravity IDE Language Server process is running.
    NotRunning,
    /// The process is there but its command line could not be read.
    UnreadableCommandLine,
    /// The command line has no CSRF token (unexpected for an IDE LS).
    MissingCsrfToken,
    /// No plain-HTTP gRPC listener was found for the process.
    NoHttpListener,
}

/// Finds the running Antigravity IDE Language Server and its gRPC endpoint.
pub fn discover() -> Result<LanguageServer, LsDiscoveryError> {
    let pid = find_ls_pid().ok_or(LsDiscoveryError::NotRunning)?;
    let cmdline = read_command_line(pid).ok_or(LsDiscoveryError::UnreadableCommandLine)?;
    let csrf_token = parse_csrf(&cmdline).ok_or(LsDiscoveryError::MissingCsrfToken)?;
    let ports = listener_ports(pid);
    if ports.is_empty() {
        return Err(LsDiscoveryError::NoHttpListener);
    }
    Ok(LanguageServer {
        pid,
        ports,
        csrf_token,
    })
}

/// PID of the first `language_server_windows_x64.exe` process, newest first.
fn find_ls_pid() -> Option<u32> {
    let output = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name='language_server_windows_x64.exe'\" | Sort-Object CreationDate -Descending | Select-Object -First 1 -ExpandProperty ProcessId",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim().parse().ok()
}

/// Full command line of a process.
fn read_command_line(pid: u32) -> Option<String> {
    let output = hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("(Get-CimInstance Win32_Process -Filter 'ProcessId={pid}').CommandLine"),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extracts the `--csrf_token` value from the LS command line.
fn parse_csrf(cmdline: &str) -> Option<String> {
    let marker = "--csrf_token ";
    let start = cmdline.find(marker)? + marker.len();
    let rest = &cmdline[start..];
    let end = rest.find(" --").unwrap_or(rest.len());
    let token = rest[..end].trim();
    if token.is_empty() || token.len() < 8 {
        return None;
    }
    Some(token.to_string())
}

/// All 127.0.0.1 LISTENING ports owned by `pid`, in netstat order.
fn listener_ports(pid: u32) -> Vec<u16> {
    let output = hidden_command("netstat").args(["-ano"]).output().ok();
    let Some(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_listener_ports(&text, pid)
}

/// Builds a `Command` that never opens a console window.
///
/// The desktop shell is a GUI application, so a spawned console tool
/// (powershell, netstat) would flash a black window on every refresh. On
/// Windows the process is created with `CREATE_NO_WINDOW`; other platforms
/// have no console flash and the command runs as-is.
fn hidden_command(program: &str) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

/// All 127.0.0.1 LISTENING ports owned by `pid`, in netstat order.
fn parse_listener_ports(text: &str, pid: u32) -> Vec<u16> {
    let suffix = format!(" {pid}");
    let mut ports = Vec::new();
    for line in text.lines() {
        if !line.trim_end().ends_with(&suffix) {
            continue;
        }
        if !line.contains("LISTENING") {
            continue;
        }
        // TCP    127.0.0.1:49578        0.0.0.0:0              LISTENING       10836
        // Only the loopback listener counts; other local addresses (0.0.0.0,
        // [::]) belong to unrelated listeners.
        let Some(tcp) = line.find("TCP") else {
            continue;
        };
        let rest = line[tcp + 3..].trim_start();
        let local = rest.split_whitespace().next().unwrap_or_default();
        if !local.starts_with("127.0.0.1:") {
            continue;
        }
        let Some(colon) = local.rfind(':') else {
            continue;
        };
        if let Ok(port) = local[colon + 1..].trim().parse::<u16>() {
            ports.push(port);
        }
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_is_parsed_from_the_ls_command_line() {
        let cmdline = r#""c:\...\language_server_windows_x64.exe" --enable_lsp --csrf_token 263f362d-2576-42e3-ad76-e571c45c1fe8 --extension_server_port 49576 --subclient_type ide"#;
        assert_eq!(
            parse_csrf(cmdline).as_deref(),
            Some("263f362d-2576-42e3-ad76-e571c45c1fe8")
        );
    }

    #[test]
    fn csrf_missing_or_blank_yields_nothing() {
        assert_eq!(parse_csrf("--csrf_token "), None);
        assert_eq!(parse_csrf("--csrf_token --other"), None);
        assert_eq!(parse_csrf("no token here"), None);
        assert_eq!(parse_csrf("--csrf_token abc"), None, "too short");
    }

    #[test]
    fn csrf_at_end_of_line_is_still_parsed() {
        let cmdline = "--csrf_token 263f362d-2576-42e3-ad76-e571c45c1fe8";
        assert_eq!(
            parse_csrf(cmdline).as_deref(),
            Some("263f362d-2576-42e3-ad76-e571c45c1fe8")
        );
    }

    /// Live discovery against the running Antigravity IDE. Requires the IDE
    /// to be open on this machine; run with `--ignored`.
    #[test]
    #[ignore]
    fn live_discovery_finds_the_language_server() {
        let ls = discover().expect("LS must be discoverable while the IDE runs");
        assert!(!ls.ports.is_empty());
        assert!(ls.csrf_token.len() >= 8);
        eprintln!("pid={} ports={:?} csrf={}", ls.pid, ls.ports, ls.csrf_token);
    }

    #[test]
    fn http_port_is_the_second_listener_of_the_process() {
        let sample = "\
  TCP    127.0.0.1:49578        0.0.0.0:0              LISTENING       10836
  TCP    127.0.0.1:49579        0.0.0.0:0              LISTENING       10836
  TCP    127.0.0.1:50531        0.0.0.0:0              LISTENING       10836
  TCP    127.0.0.1:9999         0.0.0.0:0              LISTENING       777
";
        let ports = parse_listener_ports(sample, 10836);
        assert_eq!(ports, vec![49578, 49579, 50531]);
        assert_eq!(ports.get(1).copied(), Some(49579));
    }
}
