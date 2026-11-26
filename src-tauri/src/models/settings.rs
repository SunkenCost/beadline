use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Locale {
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePresetId {
    #[default]
    #[serde(alias = "warm-claude")]
    WarmClay,
    LinearSlate,
    PureMono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppearanceMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RadiusScale {
    Sharp,
    #[default]
    Default,
    Soft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DensityMode {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeadlineThemeV1 {
    pub schema_version: u8,
    pub id: String,
    pub name: String,
    pub appearance: AppearanceMode,
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    #[serde(default)]
    pub preset: ThemePresetId,
    #[serde(default)]
    pub appearance: AppearanceMode,
    #[serde(default)]
    pub accent_override: Option<String>,
    #[serde(default)]
    pub radius: RadiusScale,
    #[serde(default)]
    pub density: DensityMode,
    #[serde(default)]
    pub active_custom_theme_id: Option<String>,
    #[serde(default)]
    pub custom_themes: Vec<BeadlineThemeV1>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            preset: ThemePresetId::WarmClay,
            appearance: AppearanceMode::System,
            accent_override: None,
            radius: RadiusScale::Default,
            density: DensityMode::Comfortable,
            active_custom_theme_id: None,
            custom_themes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            x: 120,
            y: 80,
            width: 1320,
            height: 840,
            maximized: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_deep_link_schemes() -> Vec<String> {
    vec![
        "t3-thread".to_string(),
        "vscode".to_string(),
        "cursor".to_string(),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub language: Locale,
    #[serde(default)]
    pub theme: ThemeSettings,
    #[serde(default)]
    pub bd_path: Option<String>,
    #[serde(default = "default_true")]
    pub use_force_writes: bool,
    #[serde(default = "default_deep_link_schemes")]
    pub deep_link_schemes: Vec<String>,
    #[serde(default)]
    pub window: Option<WindowGeometry>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Locale::ZhCn,
            theme: ThemeSettings::default(),
            bd_path: None,
            use_force_writes: true,
            deep_link_schemes: default_deep_link_schemes(),
            window: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: &'static str,
    pub arch: &'static str,
    pub version: &'static str,
}
