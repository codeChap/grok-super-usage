//! Minimal protobuf / gRPC-web walker for GetGrokCreditsConfig.
//!
//! Port of grokbar-omarchy's scanner: we do not have the .proto, so this
//! walks tags and picks the SuperGrok weekly pool fields by path.

use chrono::{TimeZone, Utc};

use crate::scan::Category;
use crate::util::to_iso;

// billing_product.proto (same mapping as the old i3-grok-usage bar):
// API=1, GROK_BUILD=2, GROK_PLUGINS=3, GROK_CHAT=4, GROK_IMAGINE=5, GROK_VOICE=6
const CATEGORY_LABELS: &[(i32, &str)] = &[
    (1, "API"),
    (2, "Grok Build"),
    (3, "Plugins"),
    (4, "Chat"),
    (5, "Imagine"),
    (6, "Voice"),
];

const CORE_CATEGORIES: &[(i32, &str)] = &[(2, "Grok Build"), (4, "Chat"), (5, "Imagine")];

pub struct CreditsConfig {
    pub used_fraction: f64,
    pub reset_iso: String,
    pub period_start_iso: String,
    pub categories: Vec<Category>,
}

struct Scan {
    fixed32: Vec<(Vec<u32>, f32, usize)>,
    varints: Vec<(Vec<u32>, u64)>,
    categories: Vec<(i32, f32)>,
}

pub fn parse_credits_config(raw: &[u8]) -> Option<CreditsConfig> {
    if raw.is_empty() {
        return None;
    }

    let mut frames = grpc_web_data_frames(raw).unwrap_or_default();
    if frames.is_empty() {
        if looks_like_protobuf(raw) {
            frames.push(raw.to_vec());
        } else {
            return None;
        }
    }

    let mut all_fixed = Vec::new();
    let mut all_varint = Vec::new();
    let mut all_cats = Vec::new();
    for payload in &frames {
        let scan = scan_protobuf(payload, 0, &[]);
        all_fixed.extend(scan.fixed32);
        all_varint.extend(scan.varints);
        all_cats.extend(scan.categories);
    }

    let mut percent_candidates: Vec<(Vec<u32>, f32, usize)> = all_fixed
        .iter()
        .filter(|(path, value, _)| {
            path.first() == Some(&1)
                && path.last() == Some(&1)
                && !path.contains(&7)
                && (0.0..=100.0).contains(value)
        })
        .cloned()
        .collect();
    percent_candidates.sort_by_key(|(path, _, order)| (path.len(), *order));
    let mut used_percent = percent_candidates.first().map(|(_, v, _)| *v);

    let now_sec = Utc::now().timestamp() as u64;
    let ts_fields: Vec<(Vec<u32>, u64)> = all_varint
        .iter()
        .filter(|(_, v)| looks_like_unix_secs(*v, now_sec))
        .cloned()
        .collect();
    let future: Vec<(Vec<u32>, u64)> = ts_fields
        .iter()
        .filter(|(_, v)| *v > now_sec)
        .cloned()
        .collect();

    let mut resets_at_sec = future
        .iter()
        .find(|(p, _)| p.as_slice() == [1, 5, 1])
        .map(|(_, v)| *v);
    if resets_at_sec.is_none() {
        resets_at_sec = future.iter().map(|(_, v)| *v).min();
    }

    let mut period_start_sec = ts_fields
        .iter()
        .find(|(p, _)| p.as_slice() == [1, 4, 1])
        .map(|(_, v)| *v);
    if period_start_sec.is_none() {
        let past: Vec<(Vec<u32>, u64)> = ts_fields
            .iter()
            .filter(|(_, v)| *v <= now_sec)
            .cloned()
            .collect();
        period_start_sec = past
            .iter()
            .find(|(p, _)| p.len() >= 2 && p[0] == 1 && p[1] == 4)
            .map(|(_, v)| *v);
        if period_start_sec.is_none() {
            if let Some(reset) = resets_at_sec {
                period_start_sec = past
                    .iter()
                    .filter(|(_, v)| *v < reset)
                    .map(|(_, v)| *v)
                    .max();
            }
        }
    }

    let has_usage_period = all_varint.iter().any(|(p, v)| {
        (p.len() >= 2 && p[0] == 1 && p[1] == 6)
            || (p.as_slice() == [1, 8, 1] && (*v == 1 || *v == 2))
    });
    if used_percent.is_none() && all_fixed.is_empty() && resets_at_sec.is_some() && has_usage_period
    {
        used_percent = Some(0.0);
    }

    if used_percent.is_some() && resets_at_sec.is_none() && !has_usage_period {
        used_percent = None;
    }

    let used_percent = used_percent?;

    let reset_iso = resets_at_sec
        .and_then(|sec| Utc.timestamp_opt(sec as i64, 0).single())
        .map(to_iso)
        .unwrap_or_default();

    let period_start_iso = if let Some(sec) = period_start_sec {
        Utc.timestamp_opt(sec as i64, 0)
            .single()
            .map(to_iso)
            .unwrap_or_default()
    } else if let Some(reset) = resets_at_sec {
        Utc.timestamp_opt(reset as i64, 0)
            .single()
            .map(|dt| to_iso(dt - chrono::Duration::days(7)))
            .unwrap_or_default()
    } else {
        String::new()
    };

    let mut categories: Vec<Category> = all_cats
        .into_iter()
        .map(|(type_id, pct)| Category {
            title: CATEGORY_LABELS
                .iter()
                .find(|(id, _)| *id == type_id)
                .map(|(_, name)| (*name).to_string())
                .unwrap_or_else(|| format!("Category {type_id}")),
            type_id,
            percent: f64::from(pct) / 100.0,
        })
        .collect();
    for (type_id, title) in CORE_CATEGORIES {
        if !categories.iter().any(|c| c.type_id == *type_id) {
            categories.push(Category {
                title: (*title).to_string(),
                type_id: *type_id,
                percent: 0.0,
            });
        }
    }
    categories.sort_by(|a, b| {
        category_order(a.type_id)
            .cmp(&category_order(b.type_id))
            .then(
                b.percent
                    .partial_cmp(&a.percent)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.title.cmp(&b.title))
    });

    Some(CreditsConfig {
        used_fraction: f64::from(used_percent) / 100.0,
        reset_iso,
        period_start_iso,
        categories,
    })
}

fn category_order(type_id: i32) -> u8 {
    match type_id {
        2 => 0, // Grok Build (B)
        4 => 1, // Chat (C)
        5 => 2, // Imagine (I)
        1 => 3, // API
        6 => 4, // Voice
        3 => 5, // Plugins
        _ => 99,
    }
}

fn looks_like_unix_secs(value: u64, now: u64) -> bool {
    const MIN: u64 = 1_577_836_800; // 2020-01-01
    const MAX: u64 = 4_102_444_800; // 2100-01-01
    (MIN..=MAX).contains(&value) || value.abs_diff(now) <= 10 * 365 * 24 * 3600
}

/// grpc-status from a gRPC-web trailer frame (HTTP 200 can still be UNAUTHENTICATED).
pub fn grpc_web_status(raw: &[u8]) -> Option<(u32, String)> {
    let mut i = 0;
    while i + 5 <= raw.len() {
        let flags = raw[i];
        let length = u32::from_be_bytes(raw[i + 1..i + 5].try_into().ok()?) as usize;
        let start = i + 5;
        let end = start.checked_add(length)?;
        if end > raw.len() {
            return None;
        }
        if flags & 0x80 != 0 {
            let text = String::from_utf8_lossy(&raw[start..end]);
            let mut code = 0u32;
            let mut message = String::new();
            for line in text.split(|c| c == '\n' || c == '\r') {
                let line = line.trim();
                if let Some(rest) = line
                    .strip_prefix("grpc-status:")
                    .or_else(|| line.strip_prefix("Grpc-Status:"))
                {
                    code = rest.trim().parse().unwrap_or(0);
                }
                if let Some(rest) = line
                    .strip_prefix("grpc-message:")
                    .or_else(|| line.strip_prefix("Grpc-Message:"))
                {
                    message = rest.trim().to_string();
                }
            }
            return Some((code, message));
        }
        i = end;
    }
    None
}

fn grpc_web_data_frames(raw: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut frames = Vec::new();
    let mut i = 0;
    while i + 5 <= raw.len() {
        let flags = raw[i];
        let length = u32::from_be_bytes(raw[i + 1..i + 5].try_into().ok()?) as usize;
        let start = i + 5;
        let end = start.checked_add(length)?;
        if end > raw.len() {
            return None;
        }
        if flags & 0x80 == 0 {
            frames.push(raw[start..end].to_vec());
        }
        i = end;
    }
    Some(frames)
}

fn looks_like_protobuf(buf: &[u8]) -> bool {
    let Some(&first) = buf.first() else {
        return false;
    };
    let field = first >> 3;
    let wire = first & 0x07;
    field > 0 && matches!(wire, 0 | 1 | 2 | 5)
}

fn read_varint(buf: &[u8], mut index: usize) -> (Option<u64>, usize) {
    let mut value: u64 = 0;
    let mut shift = 0;
    while index < buf.len() && shift < 64 {
        let b = buf[index];
        index += 1;
        value |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return (Some(value), index);
        }
        shift += 7;
    }
    (None, index)
}

fn scan_protobuf(buf: &[u8], depth: usize, path: &[u32]) -> Scan {
    let mut fixed32 = Vec::new();
    let mut varints = Vec::new();
    let mut categories = Vec::new();
    let mut index = 0;
    let mut order = 0;

    while index < buf.len() {
        let (key, next) = read_varint(buf, index);
        index = next;
        let Some(key) = key.filter(|k| *k != 0) else {
            break;
        };
        let field_number = (key >> 3) as u32;
        let wire_type = (key & 0x07) as u32;
        let mut field_path = path.to_vec();
        field_path.push(field_number);

        match wire_type {
            0 => {
                let (value, next) = read_varint(buf, index);
                index = next;
                if let Some(value) = value {
                    varints.push((field_path, value));
                } else {
                    break;
                }
            }
            1 => {
                if index + 8 > buf.len() {
                    break;
                }
                index += 8;
            }
            2 => {
                let (length, next) = read_varint(buf, index);
                index = next;
                let Some(length) = length.map(|n| n as usize) else {
                    break;
                };
                if index + length > buf.len() {
                    break;
                }
                let nested = &buf[index..index + length];
                if field_path.len() == 2 && field_path[0] == 1 && field_path[1] == 7 {
                    let nested_fields = scan_protobuf(nested, depth + 1, &field_path);
                    let mut cat_type = None;
                    let mut cat_pct = None;
                    for (p, v) in &nested_fields.varints {
                        if p.last() == Some(&1) {
                            cat_type = Some(*v as i32);
                        }
                    }
                    for (p, v, _) in &nested_fields.fixed32 {
                        if p.last() == Some(&2) && (0.0..=100.0).contains(v) {
                            cat_pct = Some(*v);
                        }
                    }
                    if let Some(type_id) = cat_type {
                        categories.push((type_id, cat_pct.unwrap_or(0.0)));
                    } else {
                        fixed32.extend(nested_fields.fixed32);
                        varints.extend(nested_fields.varints);
                    }
                } else if depth < 4 {
                    let nested_fields = scan_protobuf(nested, depth + 1, &field_path);
                    fixed32.extend(nested_fields.fixed32);
                    varints.extend(nested_fields.varints);
                    categories.extend(nested_fields.categories);
                }
                index += length;
            }
            5 => {
                if index + 4 > buf.len() {
                    break;
                }
                let mut bits = [0u8; 4];
                bits.copy_from_slice(&buf[index..index + 4]);
                let bits = u32::from_le_bytes(bits);
                let value = f32::from_bits(bits);
                fixed32.push((field_path, value, order));
                order += 1;
                index += 4;
            }
            _ => break,
        }
    }

    Scan {
        fixed32,
        varints,
        categories,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                b |= 0x80;
            }
            out.push(b);
            if value == 0 {
                break;
            }
        }
        out
    }

    fn encode_key(field: u32, wire: u32) -> Vec<u8> {
        encode_varint(u64::from((field << 3) | wire))
    }

    fn len_field(field: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = encode_key(field, 2);
        out.extend(encode_varint(payload.len() as u64));
        out.extend_from_slice(payload);
        out
    }

    fn fixed32_field(field: u32, value: f32) -> Vec<u8> {
        let mut out = encode_key(field, 5);
        out.extend_from_slice(&value.to_bits().to_le_bytes());
        out
    }

    fn varint_field(field: u32, value: u64) -> Vec<u8> {
        let mut out = encode_key(field, 0);
        out.extend(encode_varint(value));
        out
    }

    fn grpc_frame(payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0];
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn parses_weekly_pool_and_build_slice() {
        let start = (Utc::now().timestamp() as u64).saturating_sub(3600);
        let end = start + 7 * 24 * 3600;
        let mut period_start = Vec::new();
        period_start.extend(varint_field(1, start));
        let mut period_end = Vec::new();
        period_end.extend(varint_field(1, end));
        let mut category = Vec::new();
        category.extend(varint_field(1, 2));
        category.extend(fixed32_field(2, 4.0));
        let mut inner = Vec::new();
        inner.extend(fixed32_field(1, 4.0));
        inner.extend(len_field(4, &period_start));
        inner.extend(len_field(5, &period_end));
        inner.extend(len_field(7, &category));
        inner.extend(varint_field(6, 1));
        let msg = len_field(1, &inner);
        let raw = grpc_frame(&msg);

        let parsed = parse_credits_config(&raw).expect("credits");
        assert!((parsed.used_fraction - 0.04).abs() < 1e-6);
        assert_eq!(parsed.categories.len(), 3);
        assert_eq!(parsed.categories[0].title, "Grok Build");
        assert!((parsed.categories[0].percent - 0.04).abs() < 1e-6);
        assert_eq!(parsed.categories[1].title, "Chat");
        assert_eq!(parsed.categories[1].percent, 0.0);
        assert_eq!(parsed.categories[2].title, "Imagine");
        assert_eq!(parsed.categories[2].percent, 0.0);
        assert!(parsed.reset_iso.contains("T"));
        assert!(parsed.period_start_iso.contains("T"));
    }

    #[test]
    fn reads_grpc_web_trailer_status() {
        let mut trailer = b"grpc-status: 16\r\ngrpc-message: expired\r\n".to_vec();
        let mut raw = vec![0x80];
        raw.extend_from_slice(&(trailer.len() as u32).to_be_bytes());
        raw.append(&mut trailer);
        let (code, msg) = grpc_web_status(&raw).expect("trailer");
        assert_eq!(code, 16);
        assert_eq!(msg, "expired");
    }
}
