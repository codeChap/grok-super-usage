use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{Map, Value};

use crate::proto::parse_credits_config;
use crate::scan::{emit, ScanResult};
use crate::util::{
    atomic_write_json, decode_jwt, expand_path, home_dir, http_error_kind, http_status, parse_iso,
    plain_text, to_iso,
};

const USER_AGENT: &str = "codechap-grokbar/0.1";
const CREDITS_URL: &str = "https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig";
const SETTINGS_URL: &str = "https://cli-chat-proxy.grok.com/v1/settings";
const USER_URL: &str = "https://cli-chat-proxy.grok.com/v1/user";
const SUBSCRIPTIONS_URL: &str = "https://grok.com/rest/subscriptions";
const TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";

struct Creds {
    scope: String,
    token: String,
    refresh_token: String,
    expires_at: String,
    client_id: String,
    email: String,
    auth_path: PathBuf,
    auth_data: Value,
}

pub fn run(probe: bool, auth: Option<PathBuf>) -> i32 {
    let auth_path = expand_path(auth.as_deref(), home_dir().join(".grok/auth.json"));
    match load_auth(&auth_path) {
        Ok(Some(mut creds)) => {
            if probe {
                println!("ready");
                return 0;
            }
            match ensure_token(&mut creds) {
                Ok(()) => scan(&mut creds),
                Err(result) => emit(&result),
            }
        }
        Ok(None) => {
            if probe {
                println!("absent");
                0
            } else {
                emit(&ScanResult::status(
                    "Sign in to Grok",
                    "Run `grok login` to sign in. Credentials are stored in ~/.grok/auth.json.",
                ))
            }
        }
        Err(result) => {
            if probe {
                println!("absent");
                0
            } else {
                emit(&result)
            }
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        .build()
}

fn load_auth(path: &Path) -> Result<Option<Creds>, ScanResult> {
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|_| {
        ScanResult::status("Grok unavailable", "Could not read the Grok auth file.")
    })?;
    let data: Value = serde_json::from_str(&raw).map_err(|_| {
        ScanResult::status("Grok unavailable", "Could not read the Grok auth file.")
    })?;
    let obj = data.as_object().ok_or_else(|| {
        ScanResult::status(
            "Sign in to Grok",
            "Grok auth file is empty. Run `grok login`.",
        )
    })?;
    if obj.is_empty() {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "Grok auth file is empty. Run `grok login`.",
        ));
    }

    let mut preferred = Vec::new();
    let mut others = Vec::new();
    for (scope, entry) in obj {
        let Some(map) = entry.as_object() else {
            continue;
        };
        if map
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
            && map
                .get("access_token")
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
        {
            continue;
        }
        if scope.starts_with("https://auth.x.ai") {
            preferred.push((scope.clone(), entry.clone()));
        } else {
            others.push((scope.clone(), entry.clone()));
        }
    }
    preferred.append(&mut others);
    let Some((scope, entry)) = preferred.into_iter().next() else {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "No access token in ~/.grok/auth.json. Run `grok login`.",
        ));
    };
    let token = entry
        .get("key")
        .and_then(Value::as_str)
        .or_else(|| entry.get("access_token").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "No access token in ~/.grok/auth.json. Run `grok login`.",
        ));
    }
    Ok(Some(Creds {
        scope,
        refresh_token: entry
            .get("refresh_token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        expires_at: entry
            .get("expires_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        client_id: entry
            .get("oidc_client_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        email: entry
            .get("email")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        token,
        auth_path: path.to_path_buf(),
        auth_data: data,
    }))
}

fn token_is_fresh(creds: &Creds) -> bool {
    let Some(exp) = parse_iso(&creds.expires_at) else {
        return true;
    };
    exp > Utc::now() + ChronoDuration::seconds(120)
}

fn save_auth(creds: &Creds) {
    let mut data = creds.auth_data.clone();
    let entry = data
        .as_object_mut()
        .and_then(|obj| obj.get_mut(&creds.scope))
        .and_then(Value::as_object_mut);
    if let Some(entry) = entry {
        entry.insert("key".into(), Value::String(creds.token.clone()));
        if !creds.refresh_token.is_empty() {
            entry.insert(
                "refresh_token".into(),
                Value::String(creds.refresh_token.clone()),
            );
        }
        if !creds.expires_at.is_empty() {
            entry.insert("expires_at".into(), Value::String(creds.expires_at.clone()));
        }
    }
    if let Err(err) = atomic_write_json(&creds.auth_path, &data) {
        eprintln!("grokbar: could not write auth.json: {err}");
    }
}

fn refresh_token(creds: &mut Creds) -> Result<(), ScanResult> {
    if creds.refresh_token.trim().is_empty() || creds.client_id.trim().is_empty() {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "Grok session expired. Run `grok login` again.",
        ));
    }
    let response = agent()
        .post(TOKEN_URL)
        .set("Accept", "application/json")
        .send_form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", creds.refresh_token.as_str()),
            ("client_id", creds.client_id.as_str()),
        ]);

    let payload: Value = match response {
        Ok(resp) => resp
            .into_json()
            .map_err(|_| ScanResult::status("Grok limits unavailable", "Token refresh failed."))?,
        Err(ureq::Error::Status(code, _)) if matches!(code, 400 | 401 | 403) => {
            return Err(ScanResult::status(
                "Sign in to Grok",
                "Grok session expired. Run `grok login` again.",
            ));
        }
        Err(ureq::Error::Status(code, _)) => {
            return Err(ScanResult::status(
                "Grok limits unavailable",
                &format!("Token refresh failed (HTTP {code})."),
            ));
        }
        Err(_) => {
            return Err(ScanResult::status(
                "Grok limits unavailable",
                "Token refresh failed.",
            ));
        }
    };

    let access = payload
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or("");
    if access.is_empty() {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "Token refresh returned no access_token. Run `grok login`.",
        ));
    }
    creds.token = access.to_string();
    if let Some(refresh) = payload.get("refresh_token").and_then(Value::as_str) {
        creds.refresh_token = refresh.to_string();
    }
    if let Some(expires_in) = payload.get("expires_in").and_then(Value::as_i64) {
        creds.expires_at = to_iso(Utc::now() + ChronoDuration::seconds(expires_in));
    }
    save_auth(creds);
    Ok(())
}

fn ensure_token(creds: &mut Creds) -> Result<(), ScanResult> {
    if token_is_fresh(creds) {
        Ok(())
    } else {
        refresh_token(creds)
    }
}

fn auth_headers(req: ureq::Request, token: &str, content_type: Option<&str>) -> ureq::Request {
    let mut req = req
        .set("Authorization", &format!("Bearer {token}"))
        .set("x-grok-client-surface", "grok-build");
    if let Some(ct) = content_type {
        req = req.set("Content-Type", ct).set("Accept", ct);
    } else {
        req = req.set("Accept", "application/json");
    }
    req
}

fn http_get_json(url: &str, token: &str) -> Result<Value, (String, ScanResult)> {
    match auth_headers(agent().get(url), token, None).call() {
        Ok(resp) => resp.into_json().map_err(|_| {
            (
                "parse".into(),
                ScanResult::status(
                    "Grok limits unavailable",
                    "Could not parse settings response.",
                ),
            )
        }),
        Err(err) if http_error_kind(&err) == "auth" => Err((
            "auth".into(),
            ScanResult::status(
                "Sign in to Grok",
                "Grok session expired. Run `grok login` again.",
            ),
        )),
        Err(err) => {
            let help = http_status(&err)
                .map(|s| format!("Settings API returned HTTP {s}"))
                .unwrap_or_else(|| "Network error while loading usage.".into());
            Err((
                http_error_kind(&err).into(),
                ScanResult::status("Grok limits unavailable", &help),
            ))
        }
    }
}

fn fetch_weekly(token: &str) -> Result<crate::proto::CreditsConfig, (String, ScanResult)> {
    let body = [0u8, 0, 0, 0, 0];
    let response = auth_headers(
        agent().post(CREDITS_URL),
        token,
        Some("application/grpc-web+proto"),
    )
    .set("x-grpc-web", "1")
    .send_bytes(&body);

    let raw = match response {
        Ok(resp) => {
            let mut buf = Vec::new();
            resp.into_reader().read_to_end(&mut buf).map_err(|_| {
                (
                    "net".into(),
                    ScanResult::status(
                        "Grok limits unavailable",
                        "Network error while loading usage.",
                    ),
                )
            })?;
            buf
        }
        Err(err) if http_error_kind(&err) == "auth" => {
            return Err((
                "auth".into(),
                ScanResult::status(
                    "Sign in to Grok",
                    "Grok session expired. Run `grok login` again.",
                ),
            ));
        }
        Err(err) => {
            let help = http_status(&err)
                .map(|s| format!("Credits API returned HTTP {s}"))
                .unwrap_or_else(|| "Network error while loading usage.".into());
            return Err((
                http_error_kind(&err).into(),
                ScanResult::status("Grok limits unavailable", &help),
            ));
        }
    };

    parse_credits_config(&raw).ok_or_else(|| {
        (
            "parse".into(),
            ScanResult::status(
                "Grok limits unavailable",
                "Could not parse SuperGrok credits response.",
            ),
        )
    })
}

fn with_auth_retry<T, F>(creds: &mut Creds, mut fetch: F) -> Result<T, ScanResult>
where
    F: FnMut(&str) -> Result<T, (String, ScanResult)>,
{
    match fetch(&creds.token) {
        Ok(value) => Ok(value),
        Err((kind, _err)) if kind == "auth" => {
            refresh_token(creds)?;
            fetch(&creds.token).map_err(|(_, err)| err)
        }
        Err((_, err)) => Err(err),
    }
}

fn fetch_tier_label(token: &str) -> Result<String, (String, ScanResult)> {
    let payload = http_get_json(SETTINGS_URL, token)?;
    Ok(payload
        .get("subscription_tier_display")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string())
}

fn jwt_tier_fallback(token: &str) -> String {
    let Some(payload) = decode_jwt(token) else {
        return String::new();
    };
    for key in [
        "subscription_tier_display",
        "subscription_tier",
        "tier_name",
        "plan",
    ] {
        if let Some(val) = payload.get(key).and_then(Value::as_str) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    match payload.get("tier").and_then(Value::as_i64) {
        Some(5) => "SuperGrok Heavy".into(),
        _ => String::new(),
    }
}

fn account_display_name(payload: &Value) -> String {
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

fn fetch_account_profile(token: &str) -> Result<Value, (String, ScanResult)> {
    http_get_json(USER_URL, token)
}

fn fetch_subscriptions(token: &str) -> Result<Value, (String, ScanResult)> {
    http_get_json(SUBSCRIPTIONS_URL, token)
}

fn pick_super_grok_subscription(payload: &Value) -> Option<&Value> {
    let subs = payload.get("subscriptions")?.as_array()?;
    let mut active: Vec<&Value> = subs
        .iter()
        .filter(|sub| {
            let tier = sub.get("tier").and_then(Value::as_str).unwrap_or("");
            let status = sub.get("status").and_then(Value::as_str).unwrap_or("");
            (tier.contains("SUPER_GROK") || tier.contains("HEAVY"))
                && status == "SUBSCRIPTION_STATUS_ACTIVE"
        })
        .collect();
    if active.is_empty() {
        return None;
    }
    active.sort_by_key(|sub| {
        let stripe = sub.get("stripe").and_then(Value::as_object);
        sub.get("billingPeriodEnd")
            .and_then(Value::as_str)
            .or_else(|| stripe.and_then(|s| s.get("currentPeriodEnd").and_then(Value::as_str)))
            .unwrap_or("")
            .to_string()
    });
    active.pop()
}

fn subscription_rebill(payload: &Value) -> (String, bool) {
    let Some(sub) = pick_super_grok_subscription(payload) else {
        return (String::new(), false);
    };
    let stripe: Map<String, Value> = sub
        .get("stripe")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let end = sub
        .get("billingPeriodEnd")
        .and_then(Value::as_str)
        .or_else(|| stripe.get("currentPeriodEnd").and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string();
    let cancels = sub
        .get("cancelAtPeriodEnd")
        .and_then(Value::as_bool)
        .or_else(|| stripe.get("cancelAtPeriodEnd").and_then(Value::as_bool))
        .unwrap_or(false);
    (end, cancels)
}

fn scan(creds: &mut Creds) -> i32 {
    let weekly = match with_auth_retry(creds, fetch_weekly) {
        Ok(weekly) => weekly,
        Err(err) => return emit(&err),
    };

    let mut tier_label = String::new();
    if let Ok(tier) = with_auth_retry(creds, fetch_tier_label) {
        if !tier.is_empty() {
            tier_label = tier;
        }
    }
    if tier_label.is_empty() {
        tier_label = jwt_tier_fallback(&creds.token);
    }

    let mut account_email = String::new();
    let mut account_name = String::new();
    if let Ok(profile) = with_auth_retry(creds, fetch_account_profile) {
        account_email = profile
            .get("email")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        account_name = account_display_name(&profile);
    }
    if account_email.is_empty() {
        account_email = creds.email.clone();
    }

    let mut period_end = String::new();
    let mut cancels = false;
    if let Ok(subs) = with_auth_retry(creds, fetch_subscriptions) {
        (period_end, cancels) = subscription_rebill(&subs);
    }

    emit(&ScanResult {
        ready: true,
        rate_limit_percent: weekly.used_fraction,
        rate_limit_label: "Weekly".into(),
        rate_limit_reset_at: weekly.reset_iso,
        rate_limit_period_start: weekly.period_start_iso,
        secondary_rate_limit_percent: -1.0,
        tier_label: plain_text(&tier_label, 80),
        account_name: plain_text(&account_name, 80),
        account_email: plain_text(&account_email, 254),
        subscription_period_end: period_end,
        subscription_cancels_at_end: cancels,
        categories: weekly.categories,
        ..ScanResult::default()
    })
}
