use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

const USER_AGENT: &str = "codechap-grokbar/0.1";

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
    let compact: String = text.to_lowercase().split_whitespace().collect();
    if RESOURCE_MARKUP.iter().any(|tag| compact.contains(tag)) {
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

pub fn atomic_write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".auth.{}.{}.tmp",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    {
        let mut file = fs::File::create(&tmp)?;
        serde_json::to_writer_pretty(&mut file, value).map_err(std::io::Error::other)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, path)?;
    Ok(())
}

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
    ureq::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        .build()
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
