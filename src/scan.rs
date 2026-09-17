use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub title: String,
    #[serde(rename = "type")]
    pub type_id: i32,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub ready: bool,
    pub rate_limit_percent: f64,
    pub rate_limit_label: String,
    pub rate_limit_reset_at: String,
    pub rate_limit_period_start: String,
    pub secondary_rate_limit_percent: f64,
    pub secondary_rate_limit_label: String,
    pub secondary_rate_limit_reset_at: String,
    pub tier_label: String,
    pub account_name: String,
    pub account_email: String,
    /// xAI user id from auth.json (`user_id` / `principal_id`). Stable across email display.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub account_user_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subscription_period_end: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub subscription_cancels_at_end: bool,
    pub usage_status_text: String,
    pub auth_help_text: String,
    pub categories: Vec<Category>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub x_login_found: bool,
    pub prepaid_credits: u64,
    /// True when this row came from a saved auth snapshot, not the live CLI login.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub saved: bool,
    /// Absolute path of a saved snapshot. Empty for the live ~/.grok/auth.json login.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub saved_path: String,
}

impl Default for ScanResult {
    fn default() -> Self {
        Self {
            ready: true,
            rate_limit_percent: -1.0,
            rate_limit_label: String::new(),
            rate_limit_reset_at: String::new(),
            rate_limit_period_start: String::new(),
            secondary_rate_limit_percent: -1.0,
            secondary_rate_limit_label: String::new(),
            secondary_rate_limit_reset_at: String::new(),
            tier_label: String::new(),
            account_name: String::new(),
            account_email: String::new(),
            account_user_id: String::new(),
            subscription_period_end: String::new(),
            subscription_cancels_at_end: false,
            usage_status_text: String::new(),
            auth_help_text: String::new(),
            categories: Vec::new(),
            x_login_found: false,
            prepaid_credits: 0,
            saved: false,
            saved_path: String::new(),
        }
    }
}

impl ScanResult {
    pub fn status(usage: &str, help: &str) -> Self {
        Self {
            usage_status_text: usage.to_string(),
            auth_help_text: help.to_string(),
            ..Self::default()
        }
    }
}

pub fn emit(result: &ScanResult) -> i32 {
    emit_with_accounts(result, std::slice::from_ref(result))
}

/// Print one ScanResult plus an `accounts` array so the panel can repeat
/// the weekly block. Top-level fields are the live Grok CLI login.
pub fn emit_with_accounts(primary: &ScanResult, accounts: &[ScanResult]) -> i32 {
    let mut value = match serde_json::to_value(primary) {
        Ok(serde_json::Value::Object(map)) => serde_json::Value::Object(map),
        Ok(_) => {
            eprintln!("grok-super-usage: serialize: expected object");
            return 1;
        }
        Err(err) => {
            eprintln!("grok-super-usage: serialize: {err}");
            return 1;
        }
    };
    match serde_json::to_value(accounts) {
        Ok(list) => {
            if let serde_json::Value::Object(map) = &mut value {
                map.insert("accounts".into(), list);
            }
        }
        Err(err) => {
            eprintln!("grok-super-usage: serialize: {err}");
            return 1;
        }
    }
    println!("{value}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_with_accounts_includes_array() {
        let a = ScanResult {
            account_email: "a@x.ai".into(),
            rate_limit_percent: 0.9,
            ..ScanResult::default()
        };
        let b = ScanResult {
            account_email: "b@x.ai".into(),
            rate_limit_percent: 0.1,
            saved: true,
            saved_path: "/tmp/b.json".into(),
            ..ScanResult::default()
        };
        let value = serde_json::to_value(&a).unwrap();
        let mut wrapped = value;
        wrapped
            .as_object_mut()
            .unwrap()
            .insert("accounts".into(), serde_json::to_value([&a, &b]).unwrap());
        let accounts = wrapped.get("accounts").and_then(|v| v.as_array()).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[1]["accountEmail"], "b@x.ai");
        assert_eq!(accounts[1]["saved"], true);
    }
}
