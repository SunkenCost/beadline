use crate::error::{AppError, AppResult};
use crate::models::settings::{AppSettings, WindowGeometry};
use crate::models::workspace::{WorkspaceEntry, WorkspacesStoreData};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

const SETTINGS_FILE: &str = "settings.json";
const WORKSPACES_FILE: &str = "workspaces.json";

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

    pub fn workspaces_path(&self) -> PathBuf {
        self.config_dir.join(WORKSPACES_FILE)
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

    pub fn load_workspaces(&self) -> WorkspacesStoreData {
        let path = self.workspaces_path();
        if !path.exists() {
            return WorkspacesStoreData::default();
        }
        match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<WorkspacesStoreData>(&raw) {
                Ok(mut parsed) => {
                    parsed
                        .entries
                        .retain(|e| !e.path.trim().is_empty() && Path::new(&e.path).exists());
                    parsed
                }
                Err(err) => {
                    warn!("corrupted workspaces.json, falling back to empty: {err}");
                    WorkspacesStoreData::default()
                }
            },
            Err(err) => {
                warn!("failed to read workspaces.json: {err}");
                WorkspacesStoreData::default()
            }
        }
    }

    pub fn save_workspaces(&self, data: &WorkspacesStoreData) -> AppResult<()> {
        let payload = serde_json::to_string_pretty(data)
            .map_err(|e| AppError::ConfigIo(format!("failed to serialize workspaces: {e}")))?;
        atomic_write(&self.workspaces_path(), payload.as_bytes())
    }

    pub fn upsert_workspace(
        &self,
        entry: WorkspaceEntry,
        set_active: bool,
    ) -> AppResult<WorkspacesStoreData> {
        let mut data = self.load_workspaces();
        let norm = normalize_display_path(&entry.path);
        let mut updated = entry;
        updated.path = norm.clone();

        if let Some(existing) = data
            .entries
            .iter_mut()
            .find(|e| paths_equal(&e.path, &norm))
        {
            existing.module_prefix = updated.module_prefix;
            existing.module_aliases = updated.module_aliases;
            existing.pinned = updated.pinned;
        } else {
            data.entries.push(updated);
        }

        if set_active || data.active_path.is_none() {
            data.active_path = Some(norm);
        }
        self.save_workspaces(&data)?;
        Ok(data)
    }

    pub fn remove_workspace(&self, path: &str) -> AppResult<WorkspacesStoreData> {
        let mut data = self.load_workspaces();
        data.entries.retain(|e| !paths_equal(&e.path, path));
        if data
            .active_path
            .as_deref()
            .is_some_and(|a| paths_equal(a, path))
        {
            data.active_path = data.entries.first().map(|e| e.path.clone());
        }
        self.save_workspaces(&data)?;
        Ok(data)
    }

    pub fn set_active_workspace(&self, path: &str) -> AppResult<WorkspacesStoreData> {
        let mut data = self.load_workspaces();
        let norm = normalize_display_path(path);
        data.active_path = Some(norm);
        self.save_workspaces(&data)?;
        Ok(data)
    }
}

pub fn normalize_display_path(raw: &str) -> String {
    let p = Path::new(raw);
    let resolved = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let s = resolved.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{stripped}");
        }
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return stripped.to_string();
        }
    }
    s
}

pub fn paths_equal(a: &str, b: &str) -> bool {
    let na = normalize_display_path(a);
    let nb = normalize_display_path(b);
    #[cfg(windows)]
    {
        na.eq_ignore_ascii_case(&nb)
    }
    #[cfg(not(windows))]
    {
        na == nb
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

    #[test]
    fn workspaces_upsert_and_remove() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = tempfile::tempdir().expect("repo");
        let store = ConfigStore::new(dir.path().to_path_buf()).expect("store");
        let repo_str = repo.path().to_string_lossy().to_string();

        let data = store
            .upsert_workspace(WorkspaceEntry::new(repo_str.clone()), true)
            .expect("upsert");
        assert_eq!(data.entries.len(), 1);
        assert!(data.active_path.is_some());

        let after_rm = store.remove_workspace(&repo_str).expect("remove");
        assert!(after_rm.entries.is_empty());
        assert!(after_rm.active_path.is_none());
    }
}
