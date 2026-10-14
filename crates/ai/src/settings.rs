//! Provider preferences contain no secrets. Credentials belong to the operating system.
use crate::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub profiles: Vec<ProviderProfile>,
    pub active: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            profiles: vec![
                ProviderProfile::default(),
                ProviderProfile { name: "Local API".into(), kind: ProviderKind::Compatible, vision: false, ..Default::default() },
            ],
            active: 0,
        }
    }
}
pub fn directory() -> PathBuf {
    if let Some(dir) = std::env::var_os("DECKCRAFT_AI_CONFIG_DIR") {
        return dir.into();
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/DeckCraftAI")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home).join("DeckCraftAI")
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config")).join("deckcraft-ai")
    }
}
pub fn load() -> Settings {
    let path = directory().join("providers.json");
    let mut settings =
        std::fs::read(path).ok().filter(|b| b.len() < 1 << 20).and_then(|b| serde_json::from_slice::<Settings>(&b).ok()).unwrap_or_default();
    if settings.profiles.is_empty() {
        settings = Settings::default();
    }
    settings.active = settings.active.min(settings.profiles.len() - 1);
    settings
}
pub fn save(settings: &Settings) -> Result<(), String> {
    let dir = directory();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let temp = dir.join("providers.json.saving");
    std::fs::write(&temp, serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(temp, dir.join("providers.json")).map_err(|e| e.to_string())
}
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn entry(profile: &ProviderProfile) -> Result<keyring::Entry, String> {
    keyring::Entry::new("DeckCraftAI", &format!("{}|{}", profile.name, profile.endpoint)).map_err(|e| e.to_string())
}
pub fn load_key(profile: &ProviderProfile) -> Option<String> {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        entry(profile).ok()?.get_password().ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = profile;
        None
    }
}
pub fn save_key(profile: &ProviderProfile, key: &str) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        entry(profile)?.set_password(key).map_err(|_| "Credential store unavailable; the key remains usable for this session".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (profile, key);
        Err("Credential store unavailable; use a session-only key".into())
    }
}
pub fn drafts() -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(directory().join("drafts"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "deckcraft"))
        .collect();
    paths.sort();
    paths.reverse();
    paths
}
