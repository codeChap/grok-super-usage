//! Saved SuperGrok logins on disk: live auth.json plus snapshot files.

use std::path::{Path, PathBuf};

use crate::util::{atomic_write_secret, expand_path, home_dir};

pub const MAX_ACCOUNTS: usize = 8;

#[derive(Debug, Clone)]
pub struct AuthSource {
    pub path: PathBuf,
    pub saved: bool,
}

pub fn default_accounts_dir() -> PathBuf {
    home_dir().join(".config/omarchy/plugins/codechap.grok-super-usage/accounts")
}

pub fn collect_auth_sources(live_path: &Path, accounts_dir: Option<&Path>) -> Vec<AuthSource> {
    let mut sources = Vec::new();
    let mut seen = Vec::new();
    if live_path.is_file() {
        push_source(&mut sources, &mut seen, live_path, false);
    }
    let Some(dir) = accounts_dir else {
        return sources;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return sources;
    };
    let mut extras: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json") && p.is_file())
        .collect();
    extras.sort();
    for path in extras {
        if sources.len() >= MAX_ACCOUNTS {
            break;
        }
        push_source(&mut sources, &mut seen, &path, true);
    }
    sources
}

pub fn copy_login(auth_path: &Path, dest_dir: &Path, stem: &str) -> i32 {
    let dest = dest_dir.join(format!("{stem}.json"));
    if same_path(auth_path, &dest) {
        return 0;
    }
    let raw = match std::fs::read(auth_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("grok-super-usage: could not read auth.json: {err}");
            return 1;
        }
    };
    if let Err(err) = atomic_write_secret(&dest, &raw) {
        eprintln!("grok-super-usage: could not write snapshot: {err}");
        return 1;
    }
    0
}

pub fn forget(path: PathBuf, dir: Option<PathBuf>) -> i32 {
    let dest_dir = expand_path(dir.as_deref(), default_accounts_dir());
    let target = expand_path(Some(path.as_path()), dest_dir.join("missing.json"));
    match path_inside_dir(&dest_dir, &target) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!(
                "grok-super-usage: refusing to delete {} (not in {})",
                target.display(),
                dest_dir.display()
            );
            return 1;
        }
        Err(err) => {
            eprintln!("grok-super-usage: {err}");
            return 1;
        }
    }
    if !target.is_file() {
        eprintln!("grok-super-usage: no saved login at {}", target.display());
        return 1;
    }
    if let Err(err) = std::fs::remove_file(&target) {
        eprintln!("grok-super-usage: could not remove snapshot: {err}");
        return 1;
    }
    println!("{}", target.display());
    0
}

pub fn snapshot_stem(email: &str, scope_or_id: &str) -> String {
    if !email.trim().is_empty() {
        return sanitize_account_filename(email.trim());
    }
    let raw = scope_or_id.trim();
    if raw.is_empty() {
        return "account".into();
    }
    let tail = raw.rsplit("::").next().unwrap_or(raw).trim();
    if tail.is_empty() {
        "account".into()
    } else {
        sanitize_account_filename(tail)
    }
}

pub fn sanitize_account_filename(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
            out.push(c.to_ascii_lowercase());
        } else if c == '@' {
            out.push_str("_at_");
        } else {
            out.push('_');
        }
    }
    let trimmed: String = out.trim_matches('_').chars().take(80).collect();
    if trimmed.is_empty() {
        "account".into()
    } else {
        trimmed
    }
}

fn push_source(sources: &mut Vec<AuthSource>, seen: &mut Vec<PathBuf>, path: &Path, saved: bool) {
    if sources.len() >= MAX_ACCOUNTS {
        return;
    }
    let key = canonicalize_or_owned(path);
    if seen.iter().any(|p| p == &key) {
        return;
    }
    seen.push(key);
    sources.push(AuthSource {
        path: path.to_path_buf(),
        saved,
    });
}

fn canonicalize_or_owned(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn same_path(a: &Path, b: &Path) -> bool {
    canonicalize_or_owned(a) == canonicalize_or_owned(b)
}

fn path_inside_dir(dir: &Path, path: &Path) -> Result<bool, String> {
    let dir = dir
        .canonicalize()
        .map_err(|_| format!("accounts directory missing: {}", dir.display()))?;
    let path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return Err(format!("saved login missing: {}", path.display()));
        }
    };
    Ok(path.starts_with(&dir) && path != dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_email_to_filename() {
        assert_eq!(
            sanitize_account_filename("Derrick@CodeChap.com"),
            "derrick_at_codechap.com"
        );
        let traversal = sanitize_account_filename("../../../etc/passwd");
        assert!(!traversal.contains('/'));
        assert!(!traversal.contains('\\'));
        assert_ne!(traversal, "");
        assert_eq!(sanitize_account_filename(""), "account");
        assert_eq!(
            snapshot_stem("", "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828"),
            "b1a00492-073a-47ea-816f-4c329264a828"
        );
        assert_eq!(
            snapshot_stem("", "9bd58062-be74-4275-9b28-86df0879f42c"),
            "9bd58062-be74-4275-9b28-86df0879f42c"
        );
    }

    #[test]
    fn collect_skips_duplicate_canonical_paths_and_caps() {
        let tmp = std::env::temp_dir().join(format!(
            "grok-super-usage-collect-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let live = tmp.join("live.json");
        std::fs::write(&live, "{}").unwrap();
        for i in 0..12 {
            std::fs::write(tmp.join(format!("extra-{i}.json")), "{}").unwrap();
        }
        let sources = collect_auth_sources(&live, Some(&tmp));
        assert!(sources.len() <= MAX_ACCOUNTS);
        assert!(!sources[0].saved);
        assert!(sources.iter().skip(1).all(|s| s.saved));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn forget_refuses_path_outside_accounts_dir() {
        let tmp = std::env::temp_dir().join(format!(
            "grok-super-usage-forget-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let outside = tmp.join("outside.json");
        std::fs::write(&outside, "{}").unwrap();
        let dir = tmp.join("accounts");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(forget(outside.clone(), Some(dir.clone())), 1);
        assert!(outside.is_file());
        let inside = dir.join("keep.json");
        std::fs::write(&inside, "{}").unwrap();
        assert_eq!(forget(inside.clone(), Some(dir.clone())), 0);
        assert!(!inside.is_file());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
