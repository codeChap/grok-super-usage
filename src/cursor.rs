use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Duration as ChronoDuration, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use crate::scan::{emit, ScanResult};
use crate::util::{
    account_display_name, atomic_write_json, decode_jwt, expand_path, home_dir, http_agent,
    http_error_kind, http_status, parse_iso, plain_text, to_iso,
};

const API_BASE: &str = "https://api2.cursor.sh/aiserver.v1.DashboardService";
const TOKEN_URL: &str = "https://api2.cursor.sh/oauth/token";
const CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const CURSOR_ISS: &str = "https://authentication.cursor.sh";
const REFRESH_SKEW_SEC: i64 = 120;

const ALLOWED_JWT_TYPES: &[&str] = &["session", "web"];
const X_PROVIDERS: &[&str] = &[
    "twitter",
    "twitter-oauth",
    "twitter-oauth-2",
    "twitter-oauth2",
    "x",
    "x-oauth",
    "x-oauth2",
    "oauth_twitter",
];

struct Creds {
    source: String,
    token: String,
    refresh_token: String,
    auth_path: Option<PathBuf>,
    auth_data: Option<Value>,
    login_provider: String,
    subject: String,
    membership: String,
    email: String,
    account_name: String,
    account_email: String,
}

pub fn run(
    probe: bool,
    auth: Option<PathBuf>,
    state_db: Option<PathBuf>,
    grok_auth: Option<PathBuf>,
) -> i32 {
    let auth_path = expand_path(auth.as_deref(), home_dir().join(".config/cursor/auth.json"));
    let state_db = expand_path(
        state_db.as_deref(),
        home_dir().join(".config/Cursor/User/globalStorage/state.vscdb"),
    );
    let grok_auth = expand_path(grok_auth.as_deref(), home_dir().join(".grok/auth.json"));
    let grok_email = load_grok_email(&grok_auth);
    let creds = pick_x_credentials(&auth_path, &state_db, &grok_email);

    if probe {
        println!("{}", if creds.is_some() { "ready" } else { "absent" });
        return 0;
    }

    let Some(mut creds) = creds else {
        return emit(&ScanResult::default());
    };

    let mut allowed: Vec<&str> = X_PROVIDERS.to_vec();
    allowed.push("grok-matched");
    if !allowed.iter().any(|p| *p == creds.login_provider) {
        return emit(&ScanResult::default());
    }

    if ensure_token(&mut creds).is_err() {
        return emit(&expired_x_result());
    }
    if !allowed.iter().any(|p| *p == creds.login_provider) {
        return emit(&ScanResult::default());
    }

    let payload = match with_auth_retry(&mut creds, fetch_period_usage) {
        Ok(payload) => payload,
        Err(kind) if kind == "auth" => return emit(&expired_x_result()),
        Err(kind) => {
            return emit(&x_error_result(
                "Cursor limits unavailable",
                if kind == "net" {
                    "Could not load Cursor period usage."
                } else {
                    "Could not load Cursor period usage."
                },
            ));
        }
    };

    let tier_label = fetch_plan_name(&creds.token);
    let (account_name, account_email) = fetch_account(&creds.token);
    creds.account_name = account_name;
    creds.account_email = account_email;
    emit(&build_result(&creds, &payload, &tier_label))
}

fn expired_x_result() -> ScanResult {
    let mut out = ScanResult::status(
        "Sign in to Cursor",
        "Cursor session expired. Sign in to Cursor again.",
    );
    out.x_login_found = true;
    out
}

fn x_error_result(usage: &str, help: &str) -> ScanResult {
    let mut out = ScanResult::status(usage, help);
    out.x_login_found = true;
    out
}

fn jwt_provider_and_sub(payload: &Value) -> (String, String) {
    let sub = payload
        .get("sub")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if sub.is_empty() {
        return (String::new(), String::new());
    }
    let provider = sub
        .split_once('|')
        .map(|(p, _)| p)
        .unwrap_or("")
        .to_lowercase();
    (provider, sub)
}

fn accept_cursor_x_jwt(token: &str) -> (bool, String, String, Option<Value>) {
    let Some(payload) = decode_jwt(token) else {
        return (false, String::new(), String::new(), None);
    };
    let iss = payload
        .get("iss")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if iss != CURSOR_ISS {
        return (false, String::new(), String::new(), Some(payload));
    }
    let typ = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if !ALLOWED_JWT_TYPES.contains(&typ.as_str()) {
        return (false, String::new(), String::new(), Some(payload));
    }
    let (provider, sub) = jwt_provider_and_sub(&payload);
    let ok = X_PROVIDERS.contains(&provider.as_str());
    (ok, provider, sub, Some(payload))
}

fn normalize_email(value: &str) -> String {
    value.trim().to_lowercase()
}

fn emails_match(left: &str, right: &str) -> bool {
    let a = normalize_email(left);
    let b = normalize_email(right);
    !a.is_empty() && a == b
}

fn load_grok_email(path: &Path) -> String {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let Ok(data) = serde_json::from_str::<Value>(&raw) else {
        return String::new();
    };
    let Some(obj) = data.as_object() else {
        return String::new();
    };
    let mut preferred = Vec::new();
    let mut others = Vec::new();
    for (scope, entry) in obj {
        let Some(map) = entry.as_object() else {
            continue;
        };
        let email = normalize_email(map.get("email").and_then(Value::as_str).unwrap_or(""));
        if email.is_empty() {
            continue;
        }
        let issuer = map.get("oidc_issuer").and_then(Value::as_str).unwrap_or("");
        if scope.starts_with("https://auth.x.ai") || issuer == "https://auth.x.ai" {
            preferred.push(email);
        } else {
            others.push(email);
        }
    }
    preferred
        .into_iter()
        .chain(others)
        .next()
        .unwrap_or_default()
}

fn load_cli_profile_email(auth_path: &Path, subject: &str) -> String {
    let candidates = [
        auth_path.parent().map(|p| p.join("cli-config.json")),
        Some(home_dir().join(".config/cursor/cli-config.json")),
    ];
    for path in candidates.into_iter().flatten() {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(data) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(info) = data.get("authInfo").and_then(Value::as_object) else {
            continue;
        };
        let auth_id = info
            .get("authId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let email = info.get("email").and_then(Value::as_str).unwrap_or("");
        if !subject.is_empty() && !auth_id.is_empty() && auth_id != subject {
            continue;
        }
        if !email.is_empty() {
            return email.to_string();
        }
    }
    String::new()
}

fn cursor_session_ok(payload: &Value, email: &str, grok_email: &str) -> bool {
    let iss = payload
        .get("iss")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let typ = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if iss != CURSOR_ISS || !ALLOWED_JWT_TYPES.contains(&typ.as_str()) {
        return false;
    }
    if grok_email.is_empty() {
        return false;
    }
    emails_match(email, grok_email)
}

fn load_cli_auth(auth_path: &Path, grok_email: &str) -> Option<Creds> {
    if !auth_path.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(auth_path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    let access = data
        .get("accessToken")
        .or_else(|| data.get("access_token"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if access.is_empty() {
        return None;
    }
    let refresh = data
        .get("refreshToken")
        .or_else(|| data.get("refresh_token"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let (ok, provider, sub, payload) = accept_cursor_x_jwt(access);
    let email = load_cli_profile_email(auth_path, &sub);
    let payload = payload?;
    if !cursor_session_ok(&payload, &email, grok_email) {
        return None;
    }
    Some(Creds {
        source: "cli".into(),
        token: access.to_string(),
        refresh_token: refresh,
        auth_path: Some(auth_path.to_path_buf()),
        auth_data: Some(data),
        login_provider: if ok { provider } else { "grok-matched".into() },
        subject: sub,
        membership: String::new(),
        email,
        account_name: String::new(),
        account_email: String::new(),
    })
}

fn read_item(conn: &Connection, key: &str) -> Option<String> {
    let mut stmt = conn
        .prepare("SELECT value FROM ItemTable WHERE key = ? LIMIT 1")
        .ok()?;
    let value: rusqlite::types::Value = stmt.query_row([key], |row| row.get(0)).ok()?;
    let text = match value {
        rusqlite::types::Value::Text(text) => text,
        rusqlite::types::Value::Blob(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        rusqlite::types::Value::Integer(n) => n.to_string(),
        rusqlite::types::Value::Real(n) => n.to_string(),
        rusqlite::types::Value::Null => return None,
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn load_ide_auth(state_db: &Path, grok_email: &str) -> Option<Creds> {
    if !state_db.is_file() {
        return None;
    }
    let conn = Connection::open_with_flags(
        state_db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let _ = conn.execute_batch("PRAGMA query_only = ON");
    let access = read_item(&conn, "cursorAuth/accessToken")?;
    let refresh = read_item(&conn, "cursorAuth/refreshToken").unwrap_or_default();
    let membership = read_item(&conn, "cursorAuth/stripeMembershipType").unwrap_or_default();
    let cached_email = read_item(&conn, "cursorAuth/cachedEmail").unwrap_or_default();
    let (ok, provider, sub, payload) = accept_cursor_x_jwt(&access);
    let payload = payload?;
    if !cursor_session_ok(&payload, &cached_email, grok_email) {
        return None;
    }
    Some(Creds {
        source: "ide".into(),
        token: access,
        refresh_token: refresh,
        auth_path: None,
        auth_data: None,
        login_provider: if ok { provider } else { "grok-matched".into() },
        subject: sub,
        membership,
        email: cached_email,
        account_name: String::new(),
        account_email: String::new(),
    })
}

fn pick_x_credentials(auth_path: &Path, state_db: &Path, grok_email: &str) -> Option<Creds> {
    let cli = load_cli_auth(auth_path, grok_email);
    let ide = load_ide_auth(state_db, grok_email);
    match (cli, ide) {
        (Some(mut cli), Some(ide)) => {
            if !cli.subject.is_empty() && !ide.subject.is_empty() && cli.subject != ide.subject {
                return Some(ide);
            }
            if cli.membership.is_empty() && !ide.membership.is_empty() {
                cli.membership = ide.membership;
            }
            if cli.email.is_empty() && !ide.email.is_empty() {
                cli.email = ide.email;
            }
            Some(cli)
        }
        (cli, ide) => cli.or(ide),
    }
}

fn jwt_exp(token: &str) -> Option<DateTime<Utc>> {
    let payload = decode_jwt(token)?;
    let exp = payload
        .get("exp")?
        .as_i64()
        .or_else(|| payload.get("exp").and_then(Value::as_f64).map(|n| n as i64))?;
    Utc.timestamp_opt(exp, 0).single()
}

fn access_needs_refresh(token: &str) -> bool {
    match jwt_exp(token) {
        None => decode_jwt(token).is_none(),
        Some(exp) => exp <= Utc::now() + ChronoDuration::seconds(REFRESH_SKEW_SEC),
    }
}

fn access_is_expired(token: &str) -> bool {
    match jwt_exp(token) {
        None => decode_jwt(token).is_none(),
        Some(exp) => exp <= Utc::now(),
    }
}

fn save_cli_auth(creds: &mut Creds) {
    if creds.source != "cli" {
        return;
    }
    let Some(path) = creds.auth_path.clone() else {
        return;
    };
    let mut data = creds
        .auth_data
        .clone()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if let Some(obj) = data.as_object_mut() {
        obj.insert("accessToken".into(), Value::String(creds.token.clone()));
        if !creds.refresh_token.is_empty() {
            obj.insert(
                "refreshToken".into(),
                Value::String(creds.refresh_token.clone()),
            );
        }
    }
    creds.auth_data = Some(data.clone());
    if let Err(err) = atomic_write_json(&path, &data) {
        eprintln!("grokbar: could not write cursor auth.json: {err}");
    }
}

fn refresh_token(creds: &mut Creds) -> Result<(), &'static str> {
    if creds.refresh_token.trim().is_empty() {
        return Err("no_refresh");
    }
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": CLIENT_ID,
        "refresh_token": creds.refresh_token,
    });
    let response = http_agent()
        .post(TOKEN_URL)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_json(body);

    let payload: Value = match response {
        Ok(resp) => resp.into_json().map_err(|_| "parse")?,
        Err(ureq::Error::Status(code, _)) if matches!(code, 400 | 401 | 403) => {
            return Err("auth");
        }
        Err(ureq::Error::Status(_, _)) => return Err("http"),
        Err(_) => return Err("net"),
    };

    if payload.get("shouldLogout").and_then(Value::as_bool) == Some(true) {
        return Err("logout");
    }
    let access = payload
        .get("access_token")
        .or_else(|| payload.get("accessToken"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if access.is_empty() {
        return Err("parse");
    }
    let (ok, provider, sub, decoded) = accept_cursor_x_jwt(access);
    let login_provider = if ok {
        provider
    } else if decoded.is_some()
        && matches!(creds.login_provider.as_str(), "grok-matched" | "x-linked")
    {
        if !creds.subject.is_empty() && !sub.is_empty() && creds.subject != sub {
            return Err("logout");
        }
        "grok-matched".into()
    } else {
        return Err("logout");
    };
    creds.token = access.to_string();
    creds.login_provider = login_provider;
    creds.subject = sub;
    if let Some(refresh) = payload
        .get("refresh_token")
        .or_else(|| payload.get("refreshToken"))
        .and_then(Value::as_str)
    {
        creds.refresh_token = refresh.to_string();
    }
    save_cli_auth(creds);
    Ok(())
}

fn ensure_token(creds: &mut Creds) -> Result<(), &'static str> {
    if !access_needs_refresh(&creds.token) {
        return Ok(());
    }
    match refresh_token(creds) {
        Ok(()) => Ok(()),
        Err(reason) if reason == "logout" || access_is_expired(&creds.token) => Err(reason),
        Err(_) => Ok(()),
    }
}

fn http_post_json(url: &str, token: &str, body: Value) -> Result<Value, String> {
    let mut req = http_agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("Connect-Protocol-Version", "1");
    if !token.is_empty() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    match req.send_json(body) {
        Ok(resp) => resp.into_json().map_err(|_| "parse".into()),
        Err(err) if http_error_kind(&err) == "auth" => Err("auth".into()),
        Err(err) => {
            if let Some(status) = http_status(&err) {
                let _ = status;
                Err("http".into())
            } else {
                Err("net".into())
            }
        }
    }
}

fn fetch_period_usage(token: &str) -> Result<Value, String> {
    http_post_json(
        &format!("{API_BASE}/GetCurrentPeriodUsage"),
        token,
        serde_json::json!({}),
    )
}

fn with_auth_retry<T, F>(creds: &mut Creds, mut fetch: F) -> Result<T, String>
where
    F: FnMut(&str) -> Result<T, String>,
{
    match fetch(&creds.token) {
        Ok(value) => Ok(value),
        Err(kind) if kind == "auth" => {
            refresh_token(creds).map_err(|_| "auth".to_string())?;
            fetch(&creds.token)
        }
        Err(kind) => Err(kind),
    }
}

fn fetch_plan_name(token: &str) -> String {
    let Ok(payload) = http_post_json(
        &format!("{API_BASE}/GetPlanInfo"),
        token,
        serde_json::json!({}),
    ) else {
        return String::new();
    };
    payload
        .pointer("/planInfo/planName")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn fetch_account(token: &str) -> (String, String) {
    let Ok(payload) = http_post_json(&format!("{API_BASE}/GetMe"), token, serde_json::json!({}))
    else {
        return (String::new(), String::new());
    };
    let email = payload
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    (account_display_name(&payload), email)
}

fn parse_when(value: &Value) -> Option<DateTime<Utc>> {
    match value {
        Value::Null => None,
        Value::Bool(_) => None,
        Value::Number(n) => {
            let number = n.as_f64()?;
            if !number.is_finite() {
                return None;
            }
            let seconds = if number.abs() >= 1e11 {
                number / 1000.0
            } else {
                number
            };
            Utc.timestamp_opt(seconds as i64, 0).single()
        }
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            if let Ok(number) = text.parse::<f64>() {
                if number.is_finite() {
                    let stripped = text.trim_start_matches(['+', '-']);
                    if stripped
                        .replace('.', "")
                        .chars()
                        .all(|c| c.is_ascii_digit())
                        && stripped.matches('.').count() <= 1
                    {
                        return parse_when(&Value::from(number));
                    }
                }
            }
            parse_iso(text)
        }
        _ => None,
    }
}

fn add_months(dt: DateTime<Utc>, months: i32) -> DateTime<Utc> {
    let month_index = dt.month() as i32 - 1 + months;
    let year = dt.year() + month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) + 1;
    let last = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    };
    let day = dt.day().min(last);
    dt.with_year(year)
        .and_then(|d| d.with_month(month as u32))
        .and_then(|d| d.with_day(day))
        .unwrap_or(dt)
}

fn format_tier(value: &str) -> String {
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }
    if text.chars().any(|c| c.is_uppercase()) && text.contains(' ') {
        return text.to_string();
    }
    if text.chars().any(|c| c.is_uppercase()) && !text.contains('_') {
        return text.to_string();
    }
    text.replace('_', " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn percent_from_plan(plan: &Value, key: &str) -> f64 {
    plan.get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
        .map(|n| n / 100.0)
        .unwrap_or(0.0)
}

fn build_result(creds: &Creds, payload: &Value, tier_label: &str) -> ScanResult {
    let Some(plan) = payload.get("planUsage") else {
        return x_error_result(
            "Cursor limits unavailable",
            "Usage response did not include plan usage.",
        );
    };
    let reset_dt = payload.get("billingCycleEnd").and_then(parse_when);
    let mut start_dt = payload.get("billingCycleStart").and_then(parse_when);
    if start_dt.is_none() {
        if let Some(reset) = reset_dt {
            start_dt = Some(add_months(reset, -1));
        }
    }
    let membership = if !tier_label.is_empty() {
        tier_label.to_string()
    } else if !creds.membership.is_empty() {
        creds.membership.clone()
    } else {
        payload
            .get("membershipType")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };

    ScanResult {
        ready: true,
        rate_limit_percent: percent_from_plan(plan, "autoPercentUsed"),
        rate_limit_label: "Cursor Models".into(),
        rate_limit_reset_at: reset_dt.map(to_iso).unwrap_or_default(),
        rate_limit_period_start: start_dt.map(to_iso).unwrap_or_default(),
        secondary_rate_limit_percent: percent_from_plan(plan, "apiPercentUsed"),
        secondary_rate_limit_label: "Other Models".into(),
        secondary_rate_limit_reset_at: reset_dt.map(to_iso).unwrap_or_default(),
        tier_label: plain_text(&format_tier(&membership), 80),
        account_name: plain_text(&creds.account_name, 80),
        account_email: plain_text(&creds.account_email, 254),
        x_login_found: true,
        ..ScanResult::default()
    }
}
