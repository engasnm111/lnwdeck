//! Detailed Antigravity quota via the IDE Language Server.
//!
//! Google only publishes the weekly / 5-hour model-group quota (the numbers
//! the Antigravity IDE shows on Settings -> Models) to the IDE's own language
//! server. When the IDE is running, that server listens on a localhost port
//! and answers `RetrieveUserQuotaSummary` over plain-HTTP gRPC, using a CSRF
//! token printed on its own command line. This module discovers the server
//! and translates its response into quota windows, mirroring the IDE exactly.
//!
//! Everything is read-only: the IDE is never started, stopped or modified,
//! and no credential is read from it — the CSRF token is not a secret (it is
//! only meant to keep other local processes out) and never leaves the machine.

use lnwdeck_domain::{Confidence, QuotaKind, QuotaWindow, QuotaWindowScope};
use std::time::Duration;

/// gRPC method served by the Antigravity IDE language server.
const LS_METHOD_PATH: &str =
    "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";
/// CSRF header the IDE itself sends on every gRPC call.
const CSRF_HEADER: &str = "x-codeium-csrf-token";

/// One quota bucket inside a model group (weekly or 5-hour window).
struct LsBucket {
    window_key: String,
    label: String,
    scope: QuotaWindowScope,
    remaining_fraction: Option<f64>,
    reset_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A model group from the LS response (Gemini Models, Claude and GPT models).
struct LsGroup {
    display_name: String,
    buckets: Vec<LsBucket>,
}

/// Calls the running Antigravity IDE language server for the quota summary.
///
/// `Err` codes are sanitized (`NOT_CONFIGURED`, `SOURCE_UNAVAILABLE`,
/// `SOURCE_SCHEMA_MISMATCH`, ...) and never contain the CSRF token, a port or
/// a pid.
pub fn fetch_ls_windows(
    ports: &[u16],
    csrf_token: &str,
    timeout: Duration,
) -> Result<Vec<QuotaWindow>, String> {
    let mut last_error = "SOURCE_UNAVAILABLE".to_string();
    for &http_port in ports {
        match fetch_from_port(http_port, csrf_token, timeout) {
            Ok(windows) if !windows.is_empty() => return Ok(windows),
            Ok(_) => {
                continue;
            }
            Err(code) => {
                last_error = code;
                continue;
            }
        }
    }
    Err(last_error)
}

fn fetch_from_port(
    http_port: u16,
    csrf_token: &str,
    timeout: Duration,
) -> Result<Vec<QuotaWindow>, String> {
    let url = format!("http://127.0.0.1:{http_port}{LS_METHOD_PATH}");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .new_agent();

    // gRPC framing: one byte of compression flag (0) + 4 bytes length.
    let request_body = vec![0u8, 0, 0, 0, 0];
    let mut response = agent
        .post(&url)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header(CSRF_HEADER, csrf_token)
        .send(request_body)
        .map_err(|error| match error {
            ureq::Error::StatusCode(status) => {
                if status == 401 {
                    "AUTH_EXPIRED"
                } else {
                    "SOURCE_SCHEMA_MISMATCH"
                }
            }
            ureq::Error::Timeout(_) => "PROVIDER_TIMEOUT",
            ureq::Error::ConnectionFailed | ureq::Error::Io(_) | ureq::Error::HostNotFound => {
                "SOURCE_UNAVAILABLE"
            }
            _ => "SOURCE_UNAVAILABLE",
        })
        .map_err(str::to_string)?;

    let raw = response
        .body_mut()
        .read_to_vec()
        .map_err(|_| "PROVIDER_UNREADABLE_BODY".to_string())?;

    // gRPC response framing: 1 flag byte + 4 length bytes + message.
    if raw.len() < 5 {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    let message_len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;
    if raw.len() < 5 + message_len {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    }
    let message = &raw[5..5 + message_len];
    windows_from_ls_message(message)
}

/// Parses the `RetrieveUserQuotaSummaryResponse` protobuf message.
///
/// Wire layout (proto3, as produced by the IDE language server):
/// ```text
/// response = {
///   1: summary (message)
/// }
/// summary = {
///   2: groups (repeated Group)
/// }
/// Group = {
///   1: buckets (repeated Bucket)
///   2: display_name (string)
/// }
/// Bucket = {
///   1: bucket_id (string)
///   2: display_name (string)
///   3: window (string)              // "weekly" | "5h"
///   4: remaining_fraction (float)
///   6: reset_time (Timestamp)       // {1: seconds}
/// }
/// ```
fn windows_from_ls_message(message: &[u8]) -> Result<Vec<QuotaWindow>, String> {
    let groups = parse_ls_groups(message)?;
    if groups.is_empty() {
        return Err("QUOTA_NOT_PUBLISHED".to_string());
    }
    let mut windows = Vec::new();
    for group in groups {
        for bucket in group.buckets {
            let Some(remaining_fraction) = bucket.remaining_fraction else {
                continue;
            };
            if !remaining_fraction.is_finite() || !(0.0..=1.0).contains(&remaining_fraction) {
                continue;
            }
            let used_percent = 100.0 - remaining_fraction * 100.0;
            // The bucket label alone ("Weekly Limit Remaining") repeats across
            // model groups, so the group name ("Gemini Models", "Claude and
            // GPT models") is always included — the user must be able to tell
            // which group a window belongs to.
            let label = if bucket.label.is_empty() {
                format!("{} · {}", group.display_name, bucket.window_key)
            } else {
                format!("{} · {}", group.display_name, bucket.label)
            };
            windows.push(QuotaWindow::from_percent(
                bucket.window_key,
                label,
                bucket.scope,
                QuotaKind::Requests,
                used_percent,
                bucket.reset_at,
                Confidence::High,
            ));
        }
    }
    if windows.is_empty() {
        return Err("QUOTA_NOT_PUBLISHED".to_string());
    }
    Ok(windows)
}

/// Reads the `Group` messages from the response.
///
/// The response wraps the summary in field 1, and the groups live on field 2
/// of that summary message.
fn parse_ls_groups(message: &[u8]) -> Result<Vec<LsGroup>, String> {
    // Find the summary message (field 1, wire type 2).
    let mut i = 0;
    while i < message.len() {
        let (field, wire_type, next) = read_key(message, i)?;
        if wire_type == 2 && field == 1 {
            let (summary, _next2) = read_length_delimited(message, next)?;
            return parse_summary_groups(summary);
        }
        i = skip_field(message, i, wire_type)?;
    }
    Err("SOURCE_SCHEMA_MISMATCH".to_string())
}

fn parse_summary_groups(summary: &[u8]) -> Result<Vec<LsGroup>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < summary.len() {
        let (field, wire_type, next) = read_key(summary, i)?;
        if wire_type == 2 && field == 2 {
            let (chunk, next2) = read_length_delimited(summary, next)?;
            out.push(parse_ls_group(chunk)?);
            i = next2;
        } else {
            i = skip_field(summary, i, wire_type)?;
        }
    }
    Ok(out)
}

/// Parses one `Group` message: buckets (1), display_name (2).
fn parse_ls_group(chunk: &[u8]) -> Result<LsGroup, String> {
    let mut display_name = String::new();
    let mut buckets = Vec::new();
    let mut i = 0;
    while i < chunk.len() {
        let (field, wire_type, next) = read_key(chunk, i)?;
        match (field, wire_type) {
            (1, 2) => {
                let (bucket, next2) = read_length_delimited(chunk, next)?;
                buckets.push(parse_ls_bucket(bucket)?);
                i = next2;
            }
            (2, 2) => {
                let (text, next2) = read_string(chunk, next)?;
                display_name = text;
                i = next2;
            }
            _ => i = skip_field(chunk, i, wire_type)?,
        }
    }
    Ok(LsGroup {
        display_name,
        buckets,
    })
}

/// Parses one `Bucket` message.
fn parse_ls_bucket(chunk: &[u8]) -> Result<LsBucket, String> {
    let mut bucket_id = String::new();
    let mut label = String::new();
    let mut window = String::new();
    let mut remaining_fraction = None;
    let mut reset_at = None;
    let mut i = 0;
    while i < chunk.len() {
        let (field, wire_type, next) = read_key(chunk, i)?;
        match (field, wire_type) {
            (1, 2) => {
                let (text, next2) = read_string(chunk, next)?;
                bucket_id = text;
                i = next2;
            }
            (2, 2) => {
                let (text, next2) = read_string(chunk, next)?;
                label = text;
                i = next2;
            }
            (3, 2) => {
                let (text, next2) = read_string(chunk, next)?;
                window = text;
                i = next2;
            }
            (4, 5) => {
                let value = read_fixed32(chunk, next)?;
                remaining_fraction = Some(f64::from(value));
                i = next + 4;
            }
            (6, 2) => {
                let (timestamp, next2) = read_length_delimited(chunk, next)?;
                reset_at = parse_timestamp(timestamp);
                i = next2;
            }
            (7, 2) => {
                // Description — informational only, not needed for the window.
                let (_text, next2) = read_string(chunk, next)?;
                i = next2;
            }
            _ => i = skip_field(chunk, i, wire_type)?,
        }
    }
    let scope = match window.as_str() {
        "5h" => QuotaWindowScope::Rolling,
        _ => QuotaWindowScope::Weekly,
    };
    Ok(LsBucket {
        window_key: if bucket_id.is_empty() {
            window
        } else {
            bucket_id
        },
        label,
        scope,
        remaining_fraction,
        reset_at,
    })
}

/// Timestamp = { 1: seconds (int64) }.
fn parse_timestamp(chunk: &[u8]) -> Option<chrono::DateTime<chrono::Utc>> {
    let mut seconds = 0i64;
    let mut i = 0;
    while i < chunk.len() {
        let (field, wire_type, next) = read_key(chunk, i).ok()?;
        if field == 1 && wire_type == 0 {
            seconds = read_varint_i64(chunk, next).ok()?.0;
            break;
        }
        i = skip_field(chunk, i, wire_type).ok()?;
    }
    chrono::DateTime::from_timestamp(seconds, 0)
}

fn read_key(data: &[u8], i: usize) -> Result<(u64, u8, usize), String> {
    let (key, next) = read_varint(data, i)?;
    Ok((key >> 3, (key & 0x7) as u8, next))
}

fn read_varint(data: &[u8], mut i: usize) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *data
            .get(i)
            .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
        i += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i));
        }
        shift += 7;
        if shift > 63 {
            return Err("SOURCE_SCHEMA_MISMATCH".to_string());
        }
    }
}

fn read_varint_i64(data: &[u8], i: usize) -> Result<(i64, usize), String> {
    let (value, next) = read_varint(data, i)?;
    Ok((value as i64, next))
}

fn read_length_delimited(data: &[u8], i: usize) -> Result<(&[u8], usize), String> {
    let (len, next) = read_varint(data, i)?;
    let end = next
        .checked_add(len as usize)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    let chunk = data
        .get(next..end)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    Ok((chunk, end))
}

fn read_string(data: &[u8], i: usize) -> Result<(String, usize), String> {
    let (chunk, next) = read_length_delimited(data, i)?;
    let text =
        String::from_utf8(chunk.to_vec()).map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    Ok((text, next))
}

fn read_fixed32(data: &[u8], i: usize) -> Result<f32, String> {
    let bytes: [u8; 4] = data
        .get(i..i + 4)
        .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string())?
        .try_into()
        .map_err(|_| "SOURCE_SCHEMA_MISMATCH".to_string())?;
    Ok(f32::from_le_bytes(bytes))
}

fn skip_field(data: &[u8], i: usize, wire_type: u8) -> Result<usize, String> {
    match wire_type {
        0 => read_varint(data, i).map(|(_, next)| next),
        1 => data
            .get(i..i + 8)
            .map(|_| i + 8)
            .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string()),
        2 => read_length_delimited(data, i).map(|(_, next)| next),
        5 => data
            .get(i..i + 4)
            .map(|_| i + 4)
            .ok_or_else(|| "SOURCE_SCHEMA_MISMATCH".to_string()),
        // Unknown wire types (6/7) mark trailing non-field data; treat the
        // message as complete rather than failing the whole parse.
        _ => Ok(data.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a realistic LS response: two groups, weekly + 5h buckets.
    fn build_sample_response() -> Vec<u8> {
        let gemini_weekly = bucket(
            "gemini-weekly",
            "Weekly Limit Remaining",
            "weekly",
            0.8487,
            1786503430,
        );
        let gemini_5h = bucket(
            "gemini-5h",
            "Five Hour Limit Remaining",
            "5h",
            1.0,
            1786368442,
        );
        let gemini_group = group(&[gemini_weekly, gemini_5h], "Gemini Models");

        let gpt_weekly = bucket(
            "3p-weekly",
            "Weekly Limit Remaining",
            "weekly",
            0.6363,
            1786382944,
        );
        let gpt_5h = bucket("3p-5h", "Five Hour Limit Remaining", "5h", 1.0, 1786368442);
        let gpt_group = group(&[gpt_weekly, gpt_5h], "Claude and GPT models");

        // summary message: field 2 = repeated groups
        let mut summary = Vec::new();
        for g in [gemini_group, gpt_group] {
            summary.push(0x12); // field 2, wire type 2
            push_varint(&mut summary, g.len() as u64);
            summary.extend_from_slice(&g);
        }
        // response: field 1 = summary
        let mut message = Vec::new();
        message.push(0x0A);
        push_varint(&mut message, summary.len() as u64);
        message.extend_from_slice(&summary);
        message
    }

    fn bucket(id: &str, label: &str, window: &str, fraction: f32, reset_secs: i64) -> Vec<u8> {
        let mut b = Vec::new();
        push_string(&mut b, 1, id);
        push_string(&mut b, 2, label);
        push_string(&mut b, 3, window);
        // field 4 fixed32
        b.push(0x25);
        b.extend_from_slice(&fraction.to_le_bytes());
        // field 6 timestamp { field 1: seconds }
        let mut ts = Vec::new();
        ts.push(0x08);
        push_varint(&mut ts, reset_secs as u64);
        b.push(0x32);
        push_varint(&mut b, ts.len() as u64);
        b.extend_from_slice(&ts);
        b
    }

    fn group(buckets: &[Vec<u8>], name: &str) -> Vec<u8> {
        let mut g = Vec::new();
        for b in buckets {
            g.push(0x0A);
            push_varint(&mut g, b.len() as u64);
            g.extend_from_slice(b);
        }
        push_string(&mut g, 2, name);
        g
    }

    fn push_string(out: &mut Vec<u8>, field: u64, value: &str) {
        out.push(((field << 3) | 2) as u8);
        push_varint(out, value.len() as u64);
        out.extend_from_slice(value.as_bytes());
    }

    fn push_varint(out: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = (value & 0x7F) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
    }

    #[test]
    fn parses_groups_and_buckets_into_quota_windows() {
        let message = build_sample_response();
        let windows = windows_from_ls_message(&message).expect("parse");

        assert_eq!(windows.len(), 4);
        let gemini_weekly = windows
            .iter()
            .find(|w| w.window_key == "gemini-weekly")
            .expect("gemini weekly");
        assert_eq!(
            gemini_weekly.label, "Gemini Models · Weekly Limit Remaining",
            "the group name must be visible so weekly windows are distinguishable"
        );
        assert_eq!(gemini_weekly.scope, QuotaWindowScope::Weekly);
        let used = gemini_weekly.used_percent.expect("used percent");
        assert!((used - 15.13).abs() < 0.01, "used={used}");
        assert!(gemini_weekly.reset_at.is_some());

        let gemini_5h = windows
            .iter()
            .find(|w| w.window_key == "gemini-5h")
            .expect("gemini 5h");
        assert_eq!(gemini_5h.label, "Gemini Models · Five Hour Limit Remaining");
        assert_eq!(gemini_5h.scope, QuotaWindowScope::Rolling);
        assert_eq!(gemini_5h.used_percent, Some(0.0));

        let gpt_weekly = windows
            .iter()
            .find(|w| w.window_key == "3p-weekly")
            .expect("gpt weekly");
        assert_eq!(
            gpt_weekly.label,
            "Claude and GPT models · Weekly Limit Remaining"
        );
        let gpt_used = gpt_weekly.used_percent.expect("used percent");
        assert!((gpt_used - 36.37).abs() < 0.01, "used={gpt_used}");
        for window in &windows {
            window.check_invariants().expect("consistent");
        }
    }

    #[test]
    fn empty_or_malformed_payloads_fail_cleanly() {
        assert_eq!(
            windows_from_ls_message(&[]).unwrap_err(),
            "SOURCE_SCHEMA_MISMATCH"
        );
        assert_eq!(
            windows_from_ls_message(&[0xFF, 0xFF, 0xFF]).unwrap_err(),
            "SOURCE_SCHEMA_MISMATCH"
        );
        assert_eq!(
            windows_from_ls_message(b"\x0a\x03abc").unwrap_err(),
            "SOURCE_SCHEMA_MISMATCH"
        );
    }

    /// Live verification against the running Antigravity IDE language
    /// server. Requires the IDE to be open; run with `--ignored`.
    #[test]
    #[ignore]
    fn live_fetch_from_language_server() {
        let ls = lnwdeck_windows_integration::antigravity_ls::discover()
            .expect("LS discoverable while the IDE runs");
        eprintln!("ports={:?} csrf={}", ls.ports, ls.csrf_token);
        let windows = match fetch_ls_windows(&ls.ports, &ls.csrf_token, Duration::from_secs(15)) {
            Ok(w) => w,
            Err(e) => panic!("fetch failed: {e}"),
        };
        for w in &windows {
            eprintln!(
                "window {} {} used={:?}",
                w.window_key, w.label, w.used_percent
            );
        }
        assert!(!windows.is_empty(), "at least one window");
    }

    #[test]
    fn out_of_range_fractions_are_dropped() {
        let bad = bucket("x", "X", "weekly", 1.5, 1);
        let group = group(&[bad], "Bad");
        let mut summary = Vec::new();
        summary.push(0x12);
        push_varint(&mut summary, group.len() as u64);
        summary.extend_from_slice(&group);
        let mut message = Vec::new();
        message.push(0x0A);
        push_varint(&mut message, summary.len() as u64);
        message.extend_from_slice(&summary);
        assert_eq!(
            windows_from_ls_message(&message).unwrap_err(),
            "QUOTA_NOT_PUBLISHED"
        );
    }
}
