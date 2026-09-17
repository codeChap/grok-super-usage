use std::path::{Path, PathBuf};

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{Map, Value};

use crate::accounts::{
    collect_auth_sources, copy_login, default_accounts_dir, snapshot_stem, AuthSource,
};
use crate::proto::parse_credits_config;
use crate::scan::{emit, emit_with_accounts, ScanResult};
use crate::util::{
    account_display_name, atomic_write_json, decode_jwt, expand_path, home_dir, http_agent,
    http_error_kind, http_status, lock_exclusive, parse_iso, plain_text, read_http_body,
    read_http_json, to_iso, LimitedReadError, MAX_HTTP_BODY,
};

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
    user_id: String,
    auth_path: PathBuf,
    auth_data: Value,
}

pub fn default_auth_path() -> PathBuf {
    home_dir().join(".grok/auth.json")
}

pub fn run(probe: bool, auth: Option<PathBuf>, accounts_dir: Option<PathBuf>) -> i32 {
    let live_path = expand_path(auth.as_deref(), default_auth_path());
    let extra_dir = accounts_dir.map(|p| expand_path(Some(p.as_path()), default_accounts_dir()));
    let sources = collect_auth_sources(&live_path, extra_dir.as_deref());
    if probe {
        return probe_sources(&sources);
    }
    if sources.is_empty() {
        return emit(&ScanResult::status(
            "Sign in to Grok",
            "Run `grok login` to sign in. Credentials are stored in ~/.grok/auth.json.",
        ));
    }
    scan_sources(&sources, extra_dir.as_deref())
}

pub fn snapshot(auth: Option<PathBuf>, dir: Option<PathBuf>) -> i32 {
    let auth_path = expand_path(auth.as_deref(), default_auth_path());
    let dest_dir = expand_path(dir.as_deref(), default_accounts_dir());
    match load_auth(&auth_path) {
        Ok(None) => {
            eprintln!("grok-super-usage: no Grok login at {}", auth_path.display());
            1
        }
        Err(_) => {
            eprintln!(
                "grok-super-usage: could not read Grok login at {}",
                auth_path.display()
            );
            1
        }
        Ok(Some(creds)) => {
            let stem = snapshot_stem(&creds.email, &creds.scope);
            let dest = dest_dir.join(format!("{stem}.json"));
            let code = copy_login(&auth_path, &dest_dir, &stem);
            if code == 0 {
                println!("{}", dest.display());
            }
            code
        }
    }
}

fn probe_sources(sources: &[AuthSource]) -> i32 {
    let mut any_file = false;
    let mut any_present = false;
    let mut any_unreadable = false;
    for src in sources {
        if !src.path.is_file() {
            continue;
        }
        any_file = true;
        match load_auth(&src.path) {
            Ok(Some(_)) => any_present = true,
            Ok(None) => {}
            Err(_) => any_unreadable = true,
        }
    }
    if any_present {
        println!("present");
    } else if any_file && any_unreadable {
        println!("unreadable");
    } else {
        println!("absent");
    }
    0
}

fn scan_sources(sources: &[AuthSource], accounts_dir: Option<&Path>) -> i32 {
    let scanned: Vec<ScanResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = sources
            .iter()
            .map(|src| scope.spawn(|| scan_source(src)))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join().unwrap_or_else(|_| {
                    ScanResult::status("Grok limits unavailable", "Usage scan thread failed.")
                })
            })
            .collect()
    });
    let accounts = dedupe_accounts(sources, scanned);
    if accounts.is_empty() {
        return emit(&ScanResult::status(
            "Sign in to Grok",
            "Run `grok login` to sign in. Credentials are stored in ~/.grok/auth.json.",
        ));
    }
    remember_live_login(sources, accounts_dir, &accounts);
    let primary = pick_primary(&accounts);
    emit_with_accounts(&primary, &accounts)
}

fn remember_live_login(
    sources: &[AuthSource],
    accounts_dir: Option<&Path>,
    accounts: &[ScanResult],
) {
    let Some(dir) = accounts_dir else {
        return;
    };
    let Some(live_src) = sources.iter().find(|s| !s.saved) else {
        return;
    };
    let Some(live) = accounts.iter().find(|a| !a.saved) else {
        return;
    };
    let stem = snapshot_stem(&live.account_email, &live.account_user_id);
    let dest = dir.join(format!("{stem}.json"));
    if dest.is_file() {
        return;
    }
    let _ = copy_login(&live_src.path, dir, &stem);
}

fn scan_source(source: &AuthSource) -> ScanResult {
    let mut result = match load_auth(&source.path) {
        Ok(Some(mut creds)) => match ensure_token(&mut creds) {
            Ok(()) => scan_account(&mut creds),
            Err(err) => err,
        },
        Ok(None) => ScanResult::status(
            "Sign in to Grok",
            if source.saved {
                "This saved login is empty. Remove it in Settings, or save it again after `grok login`."
            } else {
                "Run `grok login` to sign in. Credentials are stored in ~/.grok/auth.json."
            },
        ),
        Err(err) => err,
    };
    result.saved = source.saved;
    if source.saved {
        result.saved_path = source.path.display().to_string();
        if result.usage_status_text == "Sign in to Grok"
            && result.auth_help_text.contains("`grok login` again")
        {
            result.auth_help_text =
                "This saved login expired. Remove it in Settings, or save it again after `grok login`."
                    .into();
        }
    }
    result
}

fn account_key(result: &ScanResult) -> String {
    let id = result.account_user_id.trim().to_ascii_lowercase();
    if !id.is_empty() {
        return format!("id:{id}");
    }
    let email = result.account_email.trim().to_ascii_lowercase();
    if !email.is_empty() {
        return email;
    }
    let name = result.account_name.trim().to_ascii_lowercase();
    if !name.is_empty() {
        return format!("name:{name}");
    }
    if !result.saved_path.is_empty() {
        return format!("path:{}", result.saved_path);
    }
    String::new()
}

fn dedupe_accounts(sources: &[AuthSource], scanned: Vec<ScanResult>) -> Vec<ScanResult> {
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for (i, result) in scanned.into_iter().enumerate() {
        let live = sources.get(i).is_some_and(|s| !s.saved);
        let key = account_key(&result);
        if !key.is_empty() && seen.iter().any(|k| k == &key) {
            continue;
        }
        if !key.is_empty() {
            seen.push(key);
        } else if !live && result.rate_limit_percent < 0.0 && result.usage_status_text.is_empty() {
            continue;
        }
        out.push(result);
    }
    out
}

fn pick_primary(accounts: &[ScanResult]) -> ScanResult {
    accounts
        .iter()
        .find(|a| !a.saved)
        .or_else(|| accounts.first())
        .cloned()
        .unwrap_or_else(|| {
            ScanResult::status(
                "Sign in to Grok",
                "Run `grok login` to sign in. Credentials are stored in ~/.grok/auth.json.",
            )
        })
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

    let Some((scope, entry)) = pick_auth_entry(obj) else {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "No access token in ~/.grok/auth.json. Run `grok login`.",
        ));
    };
    let token = entry_token(&entry);
    if token.is_empty() {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "No access token in ~/.grok/auth.json. Run `grok login`.",
        ));
    }
    Ok(Some(Creds {
        scope,
        refresh_token: entry_field(&entry, "refresh_token"),
        expires_at: entry_field(&entry, "expires_at"),
        client_id: entry_field(&entry, "oidc_client_id"),
        email: entry_field(&entry, "email"),
        user_id: entry_user_id(&entry, &token),
        token,
        auth_path: path.to_path_buf(),
        auth_data: data,
    }))
}

fn entry_user_id(entry: &Value, token: &str) -> String {
    let from_file = entry_field(entry, "user_id");
    if !from_file.is_empty() {
        return from_file;
    }
    let principal = entry_field(entry, "principal_id");
    if !principal.is_empty() {
        return principal;
    }
    decode_jwt(token)
        .and_then(|payload| {
            payload
                .get("sub")
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

fn entry_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn entry_token(entry: &Value) -> String {
    entry
        .get("key")
        .and_then(Value::as_str)
        .or_else(|| entry.get("access_token").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn pick_auth_entry(obj: &Map<String, Value>) -> Option<(String, Value)> {
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
    preferred.into_iter().next()
}

fn token_is_fresh(creds: &Creds) -> bool {
    let Some(exp) = parse_iso(&creds.expires_at) else {
        return false;
    };
    exp > Utc::now() + ChronoDuration::seconds(120)
}

fn save_auth(creds: &Creds) {
    let _lock = lock_exclusive(&creds.auth_path).ok();
    let mut data = std::fs::read_to_string(&creds.auth_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| creds.auth_data.clone());
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
        eprintln!("grok-super-usage: could not write auth.json: {err}");
    }
}

fn refresh_token(creds: &mut Creds) -> Result<(), ScanResult> {
    if creds.refresh_token.trim().is_empty() || creds.client_id.trim().is_empty() {
        return Err(ScanResult::status(
            "Sign in to Grok",
            "Grok session expired. Run `grok login` again.",
        ));
    }
    let response = http_agent()
        .post(TOKEN_URL)
        .set("Accept", "application/json")
        .send_form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", creds.refresh_token.as_str()),
            ("client_id", creds.client_id.as_str()),
        ]);

    let payload: Value = match response {
        Ok(resp) => match read_http_json(resp, MAX_HTTP_BODY) {
            Ok(value) => value,
            Err(LimitedReadError::TooLarge) => {
                return Err(ScanResult::status(
                    "Grok limits unavailable",
                    "Token refresh response was too large.",
                ));
            }
            Err(_) => {
                return Err(ScanResult::status(
                    "Grok limits unavailable",
                    "Token refresh failed.",
                ));
            }
        },
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
        return Ok(());
    }
    if creds.refresh_token.trim().is_empty() {
        if creds.token.trim().is_empty() {
            return Err(ScanResult::status(
                "Sign in to Grok",
                "Grok session expired. Run `grok login` again.",
            ));
        }
        return Ok(());
    }
    refresh_token(creds)
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

fn scan_http_fail(err: ureq::Error, labeled: &str) -> (String, ScanResult) {
    if http_error_kind(&err) == "auth" {
        return (
            "auth".into(),
            ScanResult::status(
                "Sign in to Grok",
                "Grok session expired. Run `grok login` again.",
            ),
        );
    }
    let help = http_status(&err)
        .map(|s| format!("{labeled} returned HTTP {s}"))
        .unwrap_or_else(|| "Network error while loading usage.".into());
    (
        http_error_kind(&err).into(),
        ScanResult::status("Grok limits unavailable", &help),
    )
}

fn http_get_json(url: &str, token: &str) -> Result<Value, (String, ScanResult)> {
    match auth_headers(http_agent().get(url), token, None).call() {
        Ok(resp) => match read_http_json(resp, MAX_HTTP_BODY) {
            Ok(value) => Ok(value),
            Err(LimitedReadError::TooLarge) => Err((
                "http".into(),
                ScanResult::status(
                    "Grok limits unavailable",
                    "Usage API response was too large.",
                ),
            )),
            Err(_) => Err((
                "parse".into(),
                ScanResult::status(
                    "Grok limits unavailable",
                    "Could not parse settings response.",
                ),
            )),
        },
        Err(err) => Err(scan_http_fail(err, "Settings API")),
    }
}

fn fetch_weekly(token: &str) -> Result<crate::proto::CreditsConfig, (String, ScanResult)> {
    let body = [0u8, 0, 0, 0, 0];
    let response = auth_headers(
        http_agent().post(CREDITS_URL),
        token,
        Some("application/grpc-web+proto"),
    )
    .set("x-grpc-web", "1")
    .send_bytes(&body);

    let raw = match response {
        Ok(resp) => match read_http_body(resp, MAX_HTTP_BODY) {
            Ok(buf) => buf,
            Err(LimitedReadError::TooLarge) => {
                return Err((
                    "http".into(),
                    ScanResult::status(
                        "Grok limits unavailable",
                        "Credits API response was too large.",
                    ),
                ));
            }
            Err(_) => {
                return Err((
                    "net".into(),
                    ScanResult::status(
                        "Grok limits unavailable",
                        "Network error while loading usage.",
                    ),
                ));
            }
        },
        Err(err) => return Err(scan_http_fail(err, "Credits API")),
    };

    if let Some((code, message)) = crate::proto::grpc_web_status(&raw) {
        if code == 16 || code == 7 {
            return Err((
                "auth".into(),
                ScanResult::status(
                    "Sign in to Grok",
                    "Grok session expired. Run `grok login` again.",
                ),
            ));
        }
        if code != 0 {
            let msg = plain_text(&message, 120);
            let help = if msg.is_empty() {
                format!("Credits API grpc-status {code}")
            } else {
                format!("Credits API grpc-status {code}: {msg}")
            };
            return Err((
                "http".into(),
                ScanResult::status("Grok limits unavailable", &help),
            ));
        }
    }

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
    // Unverified JWT claims, display only.
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

fn scan_account(creds: &mut Creds) -> ScanResult {
    let weekly = match with_auth_retry(creds, fetch_weekly) {
        Ok(weekly) => weekly,
        Err(err) => return err,
    };

    let token = creds.token.clone();
    let extras = std::thread::scope(|scope| {
        let tier_h = scope.spawn(|| fetch_tier_label(&token));
        let profile_h = scope.spawn(|| fetch_account_profile(&token));
        let subs_h = scope.spawn(|| fetch_subscriptions(&token));
        (tier_h.join(), profile_h.join(), subs_h.join())
    });

    let mut tier_label = extras
        .0
        .ok()
        .and_then(Result::ok)
        .filter(|t| !t.is_empty())
        .unwrap_or_default();
    if tier_label.is_empty() {
        tier_label = jwt_tier_fallback(&creds.token);
    }

    let mut account_email = String::new();
    let mut account_name = String::new();
    if let Ok(Ok(profile)) = extras.1 {
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
    if let Ok(Ok(subs)) = extras.2 {
        (period_end, cancels) = subscription_rebill(&subs);
    }

    ScanResult {
        ready: true,
        rate_limit_percent: weekly.used_fraction,
        rate_limit_label: "Weekly".into(),
        rate_limit_reset_at: weekly.reset_iso,
        rate_limit_period_start: weekly.period_start_iso,
        secondary_rate_limit_percent: -1.0,
        tier_label: plain_text(&tier_label, 80),
        account_name: plain_text(&account_name, 80),
        account_email: plain_text(&account_email, 254),
        account_user_id: plain_text(&creds.user_id, 80),
        subscription_period_end: plain_text(&period_end, 40),
        subscription_cancels_at_end: cancels,
        categories: weekly.categories,
        prepaid_credits: weekly.prepaid_credits,
        ..ScanResult::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_xai_scope_and_skips_empty_tokens() {
        let obj = serde_json::json!({
            "https://other.example/oidc": {"key": "other-token"},
            "https://auth.x.ai/user": {"key": "xai-token", "email": "a@b.c"},
            "empty": {"key": ""}
        });
        let (scope, entry) = pick_auth_entry(obj.as_object().unwrap()).unwrap();
        assert_eq!(scope, "https://auth.x.ai/user");
        assert_eq!(entry_token(&entry), "xai-token");
        assert_eq!(entry_field(&entry, "email"), "a@b.c");
    }

    #[test]
    fn accepts_access_token_when_key_missing() {
        let obj = serde_json::json!({
            "https://auth.x.ai/user": {"access_token": "from-access"}
        });
        let (_, entry) = pick_auth_entry(obj.as_object().unwrap()).unwrap();
        assert_eq!(entry_token(&entry), "from-access");
    }

    #[test]
    fn missing_expiry_is_not_fresh() {
        let creds = Creds {
            scope: String::new(),
            token: "t".into(),
            refresh_token: String::new(),
            expires_at: String::new(),
            client_id: String::new(),
            email: String::new(),
            user_id: String::new(),
            auth_path: PathBuf::from("/tmp/x"),
            auth_data: serde_json::json!({}),
        };
        assert!(!token_is_fresh(&creds));
    }

    #[test]
    fn dedupe_keeps_live_and_drops_same_email_snapshot() {
        let live = ScanResult {
            account_email: "a@x.ai".into(),
            rate_limit_percent: 0.2,
            ..ScanResult::default()
        };
        let saved = ScanResult {
            account_email: "A@x.ai".into(),
            rate_limit_percent: 0.9,
            saved: true,
            saved_path: "/tmp/a.json".into(),
            ..ScanResult::default()
        };
        let other = ScanResult {
            account_email: "b@x.ai".into(),
            rate_limit_percent: 0.1,
            saved: true,
            saved_path: "/tmp/b.json".into(),
            ..ScanResult::default()
        };
        let sources = vec![
            AuthSource {
                path: PathBuf::from("/tmp/live.json"),
                saved: false,
            },
            AuthSource {
                path: PathBuf::from("/tmp/a.json"),
                saved: true,
            },
            AuthSource {
                path: PathBuf::from("/tmp/b.json"),
                saved: true,
            },
        ];
        let out = dedupe_accounts(&sources, vec![live, saved, other]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].account_email, "a@x.ai");
        assert!(!out[0].saved);
        assert_eq!(out[1].account_email, "b@x.ai");
    }

    #[test]
    fn dedupe_collapses_same_user_id_even_when_emails_differ() {
        let live = ScanResult {
            account_email: "new@x.ai".into(),
            account_user_id: "user-1".into(),
            rate_limit_percent: 0.0,
            ..ScanResult::default()
        };
        let saved = ScanResult {
            account_email: "old@x.ai".into(),
            account_user_id: "user-1".into(),
            rate_limit_percent: 0.9,
            saved: true,
            saved_path: "/tmp/old.json".into(),
            ..ScanResult::default()
        };
        let sources = vec![
            AuthSource {
                path: PathBuf::from("/tmp/live.json"),
                saved: false,
            },
            AuthSource {
                path: PathBuf::from("/tmp/old.json"),
                saved: true,
            },
        ];
        let out = dedupe_accounts(&sources, vec![live, saved]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].account_email, "new@x.ai");
        assert!(!out[0].saved);
    }

    #[test]
    fn snapshot_copies_auth_named_by_email() {
        let tmp = std::env::temp_dir().join(format!(
            "grok-super-usage-snap-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let auth = tmp.join("auth.json");
        let dir = tmp.join("accounts");
        std::fs::write(
            &auth,
            serde_json::json!({
                "https://auth.x.ai::abc": {
                    "key": "tok",
                    "email": "Snap@X.AI"
                }
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(snapshot(Some(auth.clone()), Some(dir.clone())), 0);
        let dest = dir.join("snap_at_x.ai.json");
        assert!(dest.is_file());
        let copied: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&dest).unwrap()).unwrap();
        assert_eq!(copied["https://auth.x.ai::abc"]["email"], "Snap@X.AI");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pick_primary_uses_live_login_even_when_colder() {
        let live = ScanResult {
            ready: true,
            rate_limit_percent: 0.0,
            account_email: "derrick@codechap.com".into(),
            saved: false,
            ..ScanResult::default()
        };
        let saved = ScanResult {
            ready: true,
            rate_limit_percent: 1.0,
            account_email: "hello@codechap.com".into(),
            saved: true,
            saved_path: "/tmp/hello.json".into(),
            ..ScanResult::default()
        };
        let primary = pick_primary(&[live, saved]);
        assert_eq!(primary.account_email, "derrick@codechap.com");
        assert!(!primary.saved);
        assert_eq!(primary.rate_limit_percent, 0.0);
    }

    #[test]
    fn remember_live_login_copies_current_cli_auth() {
        let tmp = std::env::temp_dir().join(format!(
            "grok-super-usage-remember-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let live = tmp.join("auth.json");
        let dir = tmp.join("accounts");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &live,
            serde_json::json!({
                "https://auth.x.ai::abc": {
                    "key": "tok",
                    "email": "Live@X.AI",
                    "user_id": "user-live"
                }
            })
            .to_string(),
        )
        .unwrap();
        let sources = vec![AuthSource {
            path: live.clone(),
            saved: false,
        }];
        let accounts = vec![ScanResult {
            account_email: "Live@X.AI".into(),
            account_user_id: "user-live".into(),
            saved: false,
            ..ScanResult::default()
        }];
        remember_live_login(&sources, Some(&dir), &accounts);
        let dest = dir.join("live_at_x.ai.json");
        assert!(dest.is_file(), "{}", dest.display());
        let first_mtime = std::fs::metadata(&dest).unwrap().modified().unwrap();
        std::fs::write(&live, b"changed-live-bytes").unwrap();
        remember_live_login(&sources, Some(&dir), &accounts);
        let second_mtime = std::fs::metadata(&dest).unwrap().modified().unwrap();
        assert_eq!(first_mtime, second_mtime);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
