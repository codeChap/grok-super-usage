use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

pub const USER_AGENT: &str = concat!("grok-super-usage/", env!("CARGO_PKG_VERSION"));

const RESOURCE_MARKUP: &[&str] = &[
    "<img", "<image", "<object", "<embed", "<iframe", "<frame", "<link", "<meta", "<base",
    "<source", "<svg", "<script", "<style",
];

pub fn expand_path(value: Option<&Path>, default: PathBuf) -> PathBuf {
    match value {
        None => default,
        Some(path) if path.as_os_str().is_empty() => default,
        Some(path) => {
            let text = path.to_string_lossy();
            if text == "~" {
                home_dir()
            } else if let Some(rest) = text.strip_prefix("~/") {
                home_dir().join(rest)
            } else {
                path.to_path_buf()
            }
        }
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub fn plain_text(value: &str, max_len: usize) -> String {
    let text = value.replace('\0', "").trim().to_string();
    if text.is_empty() {
        return String::new();
    }
    if contains_markup_tag(&text) {
        return String::new();
    }
    if text.len() > max_len {
        text.chars()
            .take(max_len)
            .collect::<String>()
            .trim()
            .to_string()
    } else {
        text
    }
}

fn contains_markup_tag(value: &str) -> bool {
    let lower = value.to_lowercase();
    let mut rest = lower.as_str();
    while let Some(i) = rest.find('<') {
        let after = &rest[i..];
        if RESOURCE_MARKUP.iter().any(|tag| after.starts_with(tag)) {
            return true;
        }
        rest = &rest[i + 1..];
    }
    false
}

/// True only for a raw xAI key, never for a path or filename.
pub fn looks_like_management_key(value: &str) -> bool {
    let text = value.trim();
    if text.len() < 20 || text.len() > 256 {
        return false;
    }
    if text.contains('/') || text.contains('\\') || text.contains('.') || text.contains('~') {
        return false;
    }
    if text.chars().any(char::is_whitespace) {
        return false;
    }
    text.starts_with("xai-") || text.starts_with("xai_")
}

pub fn path_segment(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn file_group_or_world_readable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o077 != 0)
        .unwrap_or(false)
}

pub struct FileLock(fs::File);

pub fn lock_exclusive(path: &Path) -> io::Result<FileLock> {
    let lock_path = path.with_extension("lock");
    if let Some(dir) = lock_path.parent() {
        fs::create_dir_all(dir)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileLock(file))
}

impl Drop for FileLock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

pub fn to_iso(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn parse_iso(value: &str) -> Option<DateTime<Utc>> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn atomic_write_json(path: &Path, value: &Value) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let mut payload = json;
    payload.push(b'\n');
    atomic_write_secret(path, &payload)
}

pub fn atomic_write_secret(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".secret.{}.{}.tmp",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Unverified JWT payload decode for display fallbacks only — never authorization.
pub fn decode_jwt(token: &str) -> Option<Value> {
    let text = token.trim();
    if text.is_empty() {
        return None;
    }
    let payload = text.split('.').nth(1)?;
    let pad = (4 - payload.len() % 4) % 4;
    let padded = format!("{payload}{}", "=".repeat(pad));
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, padded).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    value.is_object().then_some(value)
}

pub fn http_agent() -> ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT
        .get_or_init(|| {
            ureq::builder()
                .timeout(Duration::from_secs(20))
                .user_agent(USER_AGENT)
                .build()
        })
        .clone()
}

pub fn account_display_name(payload: &Value) -> String {
    if let Some(name) = payload.get("name").and_then(Value::as_str) {
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    let first = payload
        .get("firstName")
        .or_else(|| payload.get("given_name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let last = payload
        .get("lastName")
        .or_else(|| payload.get("family_name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    [first, last]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn http_error_kind(err: &ureq::Error) -> &'static str {
    match err {
        ureq::Error::Status(code, _) if matches!(code, 401 | 403) => "auth",
        ureq::Error::Status(_, _) => "http",
        _ => "net",
    }
}

pub fn http_status(err: &ureq::Error) -> Option<u16> {
    match err {
        ureq::Error::Status(code, _) => Some(*code as u16),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_img_markup() {
        assert_eq!(plain_text("<img src=x>", 80), "");
        assert_eq!(plain_text("SuperGrok Heavy", 80), "SuperGrok Heavy");
        assert_eq!(plain_text("score < 10", 80), "score < 10");
    }

    #[test]
    fn management_key_is_not_a_path() {
        assert!(looks_like_management_key(
            "xai-abcdefghijklmnopqrstuvwxyz012345"
        ));
        assert!(!looks_like_management_key("xai-mgmt-abc"));
        assert!(!looks_like_management_key("xai-mgmt.txt"));
        assert!(!looks_like_management_key("~/dev/XAI-MGMT-KEY.txt"));
        assert!(!looks_like_management_key("/home/user/XAI-MGMT-KEY.txt"));
    }

    #[test]
    fn encodes_path_segments() {
        assert_eq!(path_segment("abc-1"), "abc-1");
        assert_eq!(path_segment("a/b"), "a%2Fb");
        assert_eq!(path_segment("a b"), "a%20b");
    }

    #[test]
    fn account_name_prefers_full_name_then_given_family() {
        assert_eq!(
            account_display_name(&serde_json::json!({"name": " Ada Lovelace "})),
            "Ada Lovelace"
        );
        assert_eq!(
            account_display_name(&serde_json::json!({
                "firstName": "Ada",
                "lastName": "Lovelace"
            })),
            "Ada Lovelace"
        );
        assert_eq!(
            account_display_name(&serde_json::json!({
                "given_name": "Ada",
                "family_name": "Lovelace"
            })),
            "Ada Lovelace"
        );
        assert_eq!(account_display_name(&serde_json::json!({"name": "  "})), "");
        assert_eq!(account_display_name(&serde_json::json!({})), "");
    }
}
