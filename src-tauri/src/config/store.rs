use crate::error::{AppError, AppResult};
use crate::models::settings::{AppSettings, WindowGeometry};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone)]
pub struct ConfigStore {
    config_dir: PathBuf,
}

impl ConfigStore {
    pub fn new(config_dir: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&config_dir)
            .map_err(|e| AppError::ConfigIo(format!("failed to create config dir: {e}")))?;
        Ok(Self { config_dir })
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE)
    }

    pub fn load_settings(&self) -> AppSettings {
        let path = self.settings_path();
        if !path.exists() {
            return AppSettings::default();
        }
        match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<AppSettings>(&raw) {
                Ok(parsed) => parsed,
                Err(err) => {
                    warn!("corrupted settings.json, falling back to defaults: {err}");
                    AppSettings::default()
                }
            },
            Err(err) => {
                warn!("failed to read settings.json, falling back to defaults: {err}");
                AppSettings::default()
            }
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> AppResult<()> {
        let payload = serde_json::to_string_pretty(settings)
            .map_err(|e| AppError::ConfigIo(format!("failed to serialize settings: {e}")))?;
        atomic_write(&self.settings_path(), payload.as_bytes())
    }

    pub fn save_window_geometry(&self, geometry: WindowGeometry) -> AppResult<()> {
        let mut settings = self.load_settings();
        settings.window = Some(geometry);
        self.save_settings(&settings)
    }
}

fn atomic_write(target: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::ConfigIo("missing parent directory for config file".into()))?;
    fs::create_dir_all(parent)
        .map_err(|e| AppError::ConfigIo(format!("failed to create parent dir: {e}")))?;

    let tmp_name = format!(
        ".{}.{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config"),
        std::process::id()
    );
    let tmp_path = parent.join(tmp_name);

    fs::write(&tmp_path, bytes)
        .map_err(|e| AppError::ConfigIo(format!("failed to write temp config file: {e}")))?;

    if let Err(rename_err) = fs::rename(&tmp_path, target) {
        // On Windows, fs::rename may fail if target exists; remove then rename safely.
        #[cfg(windows)]
        {
            if target.exists() {
                let _ = fs::remove_file(target);
                if fs::rename(&tmp_path, target).is_ok() {
                    return Ok(());
                }
            }
        }
        let _ = fs::remove_file(&tmp_path);
        return Err(AppError::ConfigIo(format!(
            "failed to replace config file: {rename_err}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::{AppearanceMode, Locale, ThemePresetId};

    #[test]
    fn settings_round_trip_and_window_geometry_update() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ConfigStore::new(dir.path().to_path_buf()).expect("store");

        let defaults = store.load_settings();
        assert_eq!(defaults.language, Locale::ZhCn);
        assert_eq!(defaults.theme.preset, ThemePresetId::WarmClaude);
        assert!(defaults.use_force_writes);

        let mut updated = defaults;
        updated.language = Locale::En;
        updated.theme.preset = ThemePresetId::LinearSlate;
        updated.theme.appearance = AppearanceMode::Dark;
        store.save_settings(&updated).expect("save");

        let geo = WindowGeometry {
            x: 200,
            y: 140,
            width: 1400,
            height: 900,
            maximized: true,
        };
        store.save_window_geometry(geo).expect("save geometry");

        let reloaded = store.load_settings();
        assert_eq!(reloaded.language, Locale::En);
        assert_eq!(reloaded.theme.preset, ThemePresetId::LinearSlate);
        assert_eq!(reloaded.window, Some(geo));
    }

    #[test]
    fn corrupted_settings_falls_back_cleanly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ConfigStore::new(dir.path().to_path_buf()).expect("store");
        fs::write(store.settings_path(), "{ invalid json").expect("write corrupt");

        let loaded = store.load_settings();
        assert_eq!(loaded, AppSettings::default());
    }
}
