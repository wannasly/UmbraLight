use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::Result;
use crate::models::{ProfileStore, Settings};

const SETTINGS_FILE: &str = "settings.json";
const PROFILES_FILE: &str = "profiles.json";

/// Returns %APPDATA%\lightgui on Windows, or a fallback temporary path.
pub fn default_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("lightgui");
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        return PathBuf::from(home).join(".lightgui");
    }
    std::env::temp_dir().join("lightgui")
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE)
}

pub fn profiles_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PROFILES_FILE)
}

/// Read a JSON file, falling back to T::default() if the file is missing
/// or corrupt.
pub fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(e) => {
                eprintln!(
                    "[lightgui] corrupt json in {}: {e}; falling back to defaults",
                    path.display()
                );
                T::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => T::default(),
        Err(e) => {
            eprintln!(
                "[lightgui] failed to read {}: {e}; falling back to defaults",
                path.display()
            );
            T::default()
        }
    }
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(".tmp");
    PathBuf::from(os)
}

/// Write JSON atomically: write to <path>.tmp, then rename over destination.
/// Windows can lock files temporarily, so retry by removing destination once if rename fails.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path(path);
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp, &bytes)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = fs::remove_file(path);
            match fs::rename(&tmp, path) {
                Ok(()) => Ok(()),
                Err(e) => {
                    let _ = fs::remove_file(&tmp);
                    Err(e.into())
                }
            }
        }
    }
}

pub fn load_settings(data_dir: &Path) -> Settings {
    read_json(&settings_path(data_dir))
}

pub fn save_settings(data_dir: &Path, settings: &Settings) -> Result<()> {
    write_json(&settings_path(data_dir), settings)
}

pub fn load_profiles(data_dir: &Path) -> Result<ProfileStore> {
    Ok(read_json(&profiles_path(data_dir)))
}

pub fn save_profiles(data_dir: &Path, profiles: &ProfileStore) -> Result<()> {
    write_json(&profiles_path(data_dir), profiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lightgui-storage-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_yields_default() {
        let dir = scratch_dir();
        let settings = load_settings(&dir);
        assert_eq!(settings.mixed_port, Settings::default().mixed_port);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_yields_default() {
        let dir = scratch_dir();
        fs::write(settings_path(&dir), b"{ invalid json !!").unwrap();
        let settings = load_settings(&dir);
        assert_eq!(settings.language, Settings::default().language);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_roundtrip() {
        let dir = scratch_dir();
        let mut settings = Settings::default();
        settings.mixed_port = 8888;
        settings.language = "en".into();
        save_settings(&dir, &settings).unwrap();
        let loaded = load_settings(&dir);
        assert_eq!(loaded.mixed_port, 8888);
        assert_eq!(loaded.language, "en");
        assert!(!tmp_path(&settings_path(&dir)).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn profiles_roundtrip_creates_parent_dirs() {
        let dir = scratch_dir().join("sub").join("nested");
        let profiles = ProfileStore::default();
        save_profiles(&dir, &profiles).unwrap();
        assert!(profiles_path(&dir).exists());
        let loaded = load_profiles(&dir).unwrap();
        assert_eq!(loaded.version, 2);
        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }
}
