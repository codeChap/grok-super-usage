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
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subscription_period_end: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub subscription_cancels_at_end: bool,
    pub usage_status_text: String,
    pub auth_help_text: String,
    pub categories: Vec<Category>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub x_login_found: bool,
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
            subscription_period_end: String::new(),
            subscription_cancels_at_end: false,
            usage_status_text: String::new(),
            auth_help_text: String::new(),
            categories: Vec::new(),
            x_login_found: false,
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
    match serde_json::to_string(result) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(err) => {
            eprintln!("grok-super-usage: serialize: {err}");
            1
        }
    }
}
