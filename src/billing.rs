//! xAI Management API postpaid invoice preview (API token spend in USD).

use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::util::{
    atomic_write_secret, expand_path, file_group_or_world_readable, home_dir, http_agent,
    http_error_kind, looks_like_management_key, path_segment,
};

const BASE: &str = "https://management-api.x.ai";
const VALIDATE_URL: &str = "https://management-api.x.ai/auth/management-keys/validation";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingResult {
    pub ready: bool,
    pub amount_usd: f64,
    pub amount_cents: i64,
    pub amount_label: String,
    pub period: String,
    pub usage_status_text: String,
    pub auth_help_text: String,
}

impl Default for BillingResult {
    fn default() -> Self {
        Self {
            ready: false,
            amount_usd: -1.0,
            amount_cents: -1,
            amount_label: String::new(),
            period: String::new(),
            usage_status_text: String::new(),
            auth_help_text: String::new(),
        }
    }
}

impl BillingResult {
    fn status(usage: &str, help: &str) -> Self {
        Self {
            usage_status_text: usage.to_string(),
            auth_help_text: help.to_string(),
            ..Self::default()
        }
    }
}

pub fn default_key_file() -> PathBuf {
    home_dir().join("dev/XAI-MGMT-KEY.txt")
}

pub fn default_store_path() -> PathBuf {
    home_dir().join(".config/omarchy/plugins/codechap.grokbar/management.key")
}

#[derive(Debug)]
struct LoadedKey {
    key: String,
    warning: String,
}

pub fn run(probe: bool, key_file: Option<PathBuf>) -> i32 {
    match load_key(key_file) {
        Ok(None) => {
            if probe {
                println!("absent");
                return 0;
            }
            emit(&BillingResult::status(
                "Add a management key",
                "Put a team-scoped Management API key in a chmod 600 file and set its path in Settings, or export XAI_MANAGEMENT_KEY.",
            ))
        }
        Err(err) => {
            if probe {
                println!("unreadable");
                return 0;
            }
            emit(&err)
        }
        Ok(Some(loaded)) => {
            if probe {
                println!("present");
                return 0;
            }
            match fetch_preview(&loaded.key) {
                Ok(mut result) => {
                    if result.auth_help_text.is_empty() {
                        result.auth_help_text = loaded.warning;
                    }
                    emit(&result)
                }
                Err(err) => emit(&err),
            }
        }
    }
}

pub fn store_key(out: Option<PathBuf>) -> i32 {
    let mut raw = String::new();
    if let Err(err) = std::io::stdin().lock().read_line(&mut raw) {
        eprintln!("grokbar: could not read key from stdin: {err}");
        return 1;
    }
    let key = raw.trim();
    if !looks_like_management_key(key) {
        eprintln!("grokbar: stdin was not an xAI management key");
        return 1;
    }
    let path = expand_path(out.as_deref(), default_store_path());
    if looks_like_management_key(&path.to_string_lossy()) {
        eprintln!("grokbar: --out must be a file path");
        return 1;
    }
    if let Err(err) = atomic_write_secret(&path, format!("{key}\n").as_bytes()) {
        eprintln!("grokbar: could not write key file: {err}");
        return 1;
    }
    println!("{}", path.display());
    0
}

fn emit(result: &BillingResult) -> i32 {
    match serde_json::to_string(result) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(err) => {
            eprintln!("grokbar: billing serialize: {err}");
            1
        }
    }
}

fn load_key(explicit: Option<PathBuf>) -> Result<Option<LoadedKey>, BillingResult> {
    if let Some(raw) = explicit.as_ref() {
        let text = raw.to_string_lossy();
        if looks_like_management_key(&text) {
            return Err(BillingResult::status(
                "Pass a key file, not the key",
                "Do not put the management key on the command line. Write it to a chmod 600 file and pass --key-file, or export XAI_MANAGEMENT_KEY.",
            ));
        }
        let path = expand_path(Some(raw.as_path()), default_key_file());
        return read_key_file(&path).map(Some);
    }
    if let Ok(env) = std::env::var("XAI_MANAGEMENT_KEY") {
        let key = env.trim().to_string();
        if !key.is_empty() {
            return Ok(Some(LoadedKey {
                key,
                warning: String::new(),
            }));
        }
    }
    if let Ok(env_path) = std::env::var("XAI_MANAGEMENT_KEY_FILE") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            let path = expand_path(Some(Path::new(trimmed)), default_key_file());
            return read_key_file(&path).map(Some);
        }
    }
    let default = default_key_file();
    if default.is_file() {
        return read_key_file(&default).map(Some);
    }
    Ok(None)
}

fn read_key_file(path: &Path) -> Result<LoadedKey, BillingResult> {
    if !path.is_file() {
        return Err(BillingResult::status(
            "Management key file missing",
            &format!("No key file at {}.", path.display()),
        ));
    }
    let raw = std::fs::read_to_string(path).map_err(|_| {
        BillingResult::status(
            "Management key unreadable",
            "Could not read the management key file.",
        )
    })?;
    let key = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or("")
        .to_string();
    if key.is_empty() {
        return Err(BillingResult::status(
            "Management key file empty",
            "The management key file has no key line.",
        ));
    }
    let warning = if file_group_or_world_readable(path) {
        format!(
            "Key file {} is group- or world-readable. chmod 600 it.",
            path.display()
        )
    } else {
        String::new()
    };
    Ok(LoadedKey { key, warning })
}

fn get_json(url: &str, key: &str) -> Result<Value, BillingResult> {
    match http_agent()
        .get(url)
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .call()
    {
        Ok(resp) => resp.into_json().map_err(|_| {
            BillingResult::status(
                "API bill unavailable",
                "Could not parse Management API JSON.",
            )
        }),
        Err(err) if http_error_kind(&err) == "auth" => Err(BillingResult::status(
            "Management key rejected",
            "The management key is invalid or expired. Create a new one at console.x.ai.",
        )),
        Err(_) => Err(BillingResult::status(
            "API bill unavailable",
            "Network error talking to management-api.x.ai.",
        )),
    }
}

fn resolve_team(key: &str) -> Result<String, BillingResult> {
    let payload = get_json(VALIDATE_URL, key)?;
    let scope = payload
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_uppercase();
    let scope_id = payload
        .get("scopeId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let team_id = payload
        .get("teamId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();

    if scope.contains("ORGANIZATION") {
        if !team_id.is_empty() {
            return Ok(team_id.to_string());
        }
        return Err(BillingResult::status(
            "Management key needs a team",
            "This key is organization-scoped. Use a team-scoped management key.",
        ));
    }
    if !scope_id.is_empty() {
        return Ok(scope_id.to_string());
    }
    if !team_id.is_empty() {
        return Ok(team_id.to_string());
    }
    Err(BillingResult::status(
        "Management key needs a team",
        "Could not read a team id from the management key.",
    ))
}

fn cents_from(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f.round() as i64),
        _ => None,
    }
}

fn fetch_preview(key: &str) -> Result<BillingResult, BillingResult> {
    let team = resolve_team(key)?;
    let url = format!(
        "{BASE}/v1/billing/teams/{}/postpaid/invoice/preview",
        path_segment(&team)
    );
    let payload = get_json(&url, key)?;
    let core = payload
        .get("coreInvoice")
        .or_else(|| payload.get("core_invoice"))
        .cloned()
        .unwrap_or(payload.clone());
    let cents = core
        .get("amountAfterVat")
        .or_else(|| core.get("amount_after_vat"))
        .and_then(cents_from)
        .or_else(|| payload.get("amountAfterVat").and_then(cents_from))
        .ok_or_else(|| {
            BillingResult::status(
                "API bill unavailable",
                "Invoice preview had no amountAfterVat.",
            )
        })?;
    if cents < 0 {
        return Err(BillingResult::status(
            "API bill unavailable",
            "Invoice preview returned a negative amount.",
        ));
    }
    let usd = cents as f64 / 100.0;
    let period = payload
        .get("period")
        .or_else(|| payload.get("billingPeriod"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Ok(BillingResult {
        ready: true,
        amount_usd: usd,
        amount_cents: cents,
        amount_label: format!("${usd:.2}"),
        period,
        usage_status_text: String::new(),
        auth_help_text: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cents_from_number_or_string() {
        assert_eq!(cents_from(&serde_json::json!(6096)), Some(6096));
        assert_eq!(cents_from(&serde_json::json!("6096")), Some(6096));
    }

    #[test]
    fn refuses_inline_key_as_key_file_path() {
        let err =
            load_key(Some(PathBuf::from("xai-abcdefghijklmnopqrstuvwxyz012345"))).unwrap_err();
        assert!(err.usage_status_text.contains("file"));
    }

    #[test]
    fn explicit_missing_file_does_not_fall_through() {
        let err = load_key(Some(PathBuf::from("/no/such/grokbar-key-file"))).unwrap_err();
        assert!(err.usage_status_text.contains("missing"));
    }
}
