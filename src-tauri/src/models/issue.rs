use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageBackend {
    NativeJsonl,
    DoltCli,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBdStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub source: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardLane {
    InProgress,
    Ready,
    Blocked,
    Closed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartLink {
    pub scheme: String,
    pub url: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRef {
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartMeta {
    pub github_number: Option<u32>,
    pub github_url: Option<String>,
    pub branches: Vec<String>,
    pub commits: Vec<String>,
    pub worktrees: Vec<WorktreeRef>,
    pub links: Vec<SmartLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDto {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub description: String,
    pub notes: String,
    pub acceptance_criteria: String,
    pub close_reason: String,
    pub status: String,
    pub lane: BoardLane,
    pub priority: u8,
    pub issue_type: String,
    pub assignee: String,
    pub owner: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: String,
    pub external_ref: String,
    pub labels: Vec<String>,
    pub modules: Vec<String>,
    pub parent_id: Option<String>,
    pub parent_short_id: Option<String>,
    pub parent_title: Option<String>,
    pub children_ids: Vec<String>,
    pub blocked_by: Vec<String>,
    pub open_blocked_by: Vec<String>,
    pub blocks: Vec<String>,
    pub comment_count: usize,
    pub smart: SmartMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DagEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleCount {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardStats {
    pub total: usize,
    pub in_progress: usize,
    pub ready: usize,
    pub blocked: usize,
    pub closed: usize,
    pub epics: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub repo_name: String,
    pub repo_path: String,
    pub has_beads: bool,
    pub storage_backend: StorageBackend,
    pub prefix: Option<String>,
    pub github_repo: Option<String>,
    pub module_prefix: String,
    pub fingerprint: String,
    pub stats: BoardStats,
    pub modules: Vec<ModuleCount>,
    pub issues: Vec<IssueDto>,
    pub epics: Vec<IssueDto>,
    pub edges: Vec<DagEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentDto {
    pub id: String,
    pub author: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIssueRequest {
    pub workspace_path: String,
    pub title: String,
    pub issue_type: String,
    pub priority: u8,
    pub status: String,
    pub assignee: Option<String>,
    pub parent_id: Option<String>,
    pub labels: Vec<String>,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub notes: Option<String>,
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIssueRequest {
    pub workspace_path: String,
    pub issue_id: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<u8>,
    pub assignee: Option<String>,
    pub labels: Option<Vec<String>>,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIssueResponse {
    pub id: String,
    pub snapshot: WorkspaceSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubSyncResponse {
    pub output: String,
    pub snapshot: WorkspaceSnapshot,
}
