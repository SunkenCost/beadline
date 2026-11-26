use crate::models::issue::{IssueDto, StorageBackend};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModuleAliasConfig {
    pub label: Option<String>,
    pub color: Option<String>,
}

fn default_module_prefix() -> String {
    "mod:".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub path: String,
    #[serde(default = "default_module_prefix")]
    pub module_prefix: String,
    #[serde(default)]
    pub module_aliases: BTreeMap<String, ModuleAliasConfig>,
    #[serde(default)]
    pub pinned: bool,
}

impl WorkspaceEntry {
    pub fn new(path: String) -> Self {
        Self {
            path,
            module_prefix: default_module_prefix(),
            module_aliases: BTreeMap::new(),
            pinned: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesStoreData {
    #[serde(default)]
    pub active_path: Option<String>,
    #[serde(default)]
    pub entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub name: String,
    pub path: String,
    pub has_beads: bool,
    pub storage_backend: StorageBackend,
    pub prefix: Option<String>,
    pub issue_count: usize,
    pub in_progress_count: usize,
    pub is_registered: bool,
    pub pinned: bool,
    pub module_prefix: String,
    pub module_aliases: BTreeMap<String, ModuleAliasConfig>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResponse {
    pub active_path: Option<String>,
    pub workspaces: Vec<WorkspaceSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossProjectActiveItem {
    pub workspace_name: String,
    pub workspace_path: String,
    pub issue: IssueDto,
}
