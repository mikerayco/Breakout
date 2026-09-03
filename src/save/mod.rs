//! The JSON save profile (FR-44/45, ADR-0008).
//!
//! Owns one file at the platform config dir, `serde`-serialised, written
//! atomically (temp file in the same dir + `fsync` + rename) at run end
//! and on settings changes — never per frame. A corrupt or unknown-version
//! profile is renamed aside and replaced; play never blocks on the save.
//! `--reset-profile` wipes after an explicit `y/N` confirmation.

use std::io::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Current schema. Bumped only with a migration; unknown versions are
/// renamed aside, not parsed (ADR-0008).
pub const SCHEMA_VERSION: u32 = 1;

/// Everything persisted between launches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Schema version, first field (ADR-0008).
    pub schema_version: u32,
    /// Lifetime shard currency (FR-43).
    pub shards: u64,
    /// Finished runs.
    pub runs: u32,
    /// Best run score.
    pub best_score: u64,
    /// Mute choice (FR-48).
    pub muted: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            shards: 0,
            runs: 0,
            best_score: 0,
            muted: false,
        }
    }
}

/// Platform config path: `~/Library/Application Support/breakout/` on
/// macOS, `$XDG_CONFIG_HOME/breakout/` (else `~/.config/breakout/`) on
/// Linux (FR-44). No `dirs` crate in the budget (ADR-0005); env only.
pub fn path() -> PathBuf {
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Library/Application Support/breakout/profile.json")
    } else {
        let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            format!("{home}/.config")
        });
        PathBuf::from(base).join("breakout/profile.json")
    }
}

/// Load the profile: missing file means fresh; corrupt/unparseable/unknown
/// version is renamed to `profile.corrupt.<unix-ts>.json` and replaced.
pub fn load() -> Profile {
    load_from(&path())
}

/// Testable core of [`load`]: same policy at any path.
pub fn load_from(path: &std::path::Path) -> Profile {
    let raw = match std::fs::read_to_string(path) {
        Err(_) => return Profile::default(),
        Ok(raw) => raw,
    };
    match serde_json::from_str::<Profile>(&raw) {
        Ok(p) if p.schema_version == SCHEMA_VERSION => p,
        _ => {
            rename_aside(path);
            Profile::default()
        }
    }
}

/// Save atomically: serialise to `profile.json.tmp` in the same directory,
/// `fsync`, then rename over the target (ADR-0008). Errors are returned;
/// callers log and continue (never block play).
pub fn save(profile: &Profile) -> anyhow::Result<()> {
    save_to(&path(), profile)
}

/// Testable core of [`save`].
pub fn save_to(path: &std::path::Path, profile: &Profile) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp)?;
    let json = serde_json::to_string_pretty(profile)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Delete the profile (after confirmation, handled by the caller).
pub fn reset() -> anyhow::Result<()> {
    let path = path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Move a corrupt profile aside with a timestamp suffix.
fn rename_aside(path: &std::path::Path) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let aside = path.with_extension(format!("corrupt.{ts}.json"));
    let _ = std::fs::rename(path, aside);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "breakout-profile-test-{}-{}.json",
            std::process::id(),
            tag
        ))
    }

    #[test]
    fn roundtrip() {
        let path = tmp_path("roundtrip");
        let _ = std::fs::remove_file(&path);
        let p = Profile {
            shards: 37,
            muted: true,
            ..Default::default()
        };
        save_to(&path, &p).expect("save");
        assert_eq!(load_from(&path), p);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_is_fresh() {
        let path = tmp_path("missing-no-such-file");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_from(&path), Profile::default());
    }

    #[test]
    fn corrupt_is_renamed_aside_not_crash() {
        let path = tmp_path("corrupt");
        let _ = std::fs::remove_file(&path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("dir");
        }
        std::fs::write(&path, "garbage{{{").expect("write");
        let loaded = load_from(&path);
        assert_eq!(loaded, Profile::default());
        assert!(!path.exists(), "corrupt file must be moved aside");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn future_schema_is_renamed_aside() {
        let path = tmp_path("future");
        let _ = std::fs::remove_file(&path);
        let raw = format!(
            "{{\"schema_version\":{},\"shards\":5,\"runs\":1,\"best_score\":9,\"muted\":false}}",
            SCHEMA_VERSION + 1
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("dir");
        }
        std::fs::write(&path, raw).expect("write");
        assert_eq!(load_from(&path), Profile::default());
        assert!(!path.exists());
        let _ = std::fs::remove_file(&path);
    }
}
