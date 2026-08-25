//! xAI Management API postpaid invoice preview (API token spend in USD).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::util::{expand_path, home_dir, http_agent, http_error_kind};

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

fn key_file_candidates(explicit: Option<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(path) = explicit {
        let path = expand_path(Some(path.as_path()), default_key_file());
        if !path.as_os_str().is_empty() {
            out.push(path);
        }
    }
    if let Ok(env_path) = std::env::var("XAI_MANAGEMENT_KEY_FILE") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            out.push(expand_path(Some(Path::new(trimmed)), default_key_file()));
        }
    }
    out.push(default_key_file());
    let mut seen = std::collections::HashSet::new();
    out.into_iter().filter(|p| seen.insert(p.clone())).collect()
}

pub fn run(probe: bool, key_file: Option<PathBuf>) -> i32 {
    let key = match load_key(key_file) {
        Ok(Some(key)) => key,
        Ok(None) => {
            if probe {
                println!("absent");
                return 0;
            }
            return emit(&BillingResult::status(
                "Add a management key",
                "Put an xAI Management API key (console.x.ai) in ~/dev/XAI-MGMT-KEY.txt, or set managementKeyPath.",
            ));
        }
        Err(err) => {
            if probe {
                println!("absent");
                return 0;
            }
            return emit(&err);
        }
    };

    if probe {
        println!("ready");
        return 0;
    }

    match fetch_preview(&key) {
        Ok(result) => emit(&result),
        Err(err) => emit(&err),
    }
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

fn looks_like_key(value: &str) -> bool {
    let text = value.trim();
    text.starts_with("xai-") || text.starts_with("xai_")
}

fn load_key(explicit: Option<PathBuf>) -> Result<Option<String>, BillingResult> {
    if let Ok(env) = std::env::var("XAI_MANAGEMENT_KEY") {
        let key = env.trim().to_string();
        if !key.is_empty() {
            return Ok(Some(key));
        }
    }
    if let Some(raw) = explicit.as_ref() {
        let text = raw.to_string_lossy();
        if looks_like_key(&text) {
            return Ok(Some(text.trim().to_string()));
        }
    }
    let mut last_err = None;
    for path in key_file_candidates(explicit) {
        if !path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(raw) => {
                let key = raw
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty() && !line.starts_with('#'))
                    .unwrap_or("")
                    .to_string();
                if !key.is_empty() {
                    return Ok(Some(key));
                }
            }
            Err(_) => {
                last_err = Some(BillingResult::status(
                    "Management key unreadable",
                    "Could not read the management key file.",
                ));
            }
        }
    }
    if let Some(err) = last_err {
        return Err(err);
    }
    Ok(None)
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
    let url = format!("{BASE}/v1/billing/teams/{team}/postpaid/invoice/preview");
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
    fn detects_inline_management_keys() {
        assert!(looks_like_key("xai-mgmt-abc"));
        assert!(!looks_like_key("~/dev/XAI-MGMT-KEY.txt"));
        assert!(!looks_like_key("/home/user/XAI-MGMT-KEY.txt"));
    }
}
