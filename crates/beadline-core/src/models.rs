use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageBackend {
    NativeJsonl,
    DoltCli,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StoreKind {
    SharedGitCommon,
    LegacyBeadsDir,
    LocalBeadsDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBdStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub source: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlCliStatus {
    pub installed: bool,
    pub path: Option<String>,
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

/// Structured delivery proof reported by an agent or human when closing a task.
/// Stored inside `metadata.beadline.proof` in the authoritative store.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueProof {
    pub summary: String,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub submitted_at: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub unblocked: Vec<UnblockedSummary>,
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
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub proof: Option<IssueProof>,
    #[serde(default = "default_rev")]
    pub rev: u64,
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

fn default_rev() -> u64 {
    1
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

/// Persistent task board snapshot (decoupled from ephemeral agent leases and git scans).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub repo_id: String,
    pub repo_name: String,
    pub repo_path: String,
    pub store_path: String,
    pub store_kind: StoreKind,
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

// ---------------------------------------------------------------------------
// Live Runtime Sessions, Persistent Pending Requests & Mission Control Radar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    #[default]
    Working,
    WaitingHuman,
    Released,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TelemetrySource {
    Hook,
    #[default]
    Mcp,
    Cli,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TurnState {
    #[default]
    Running,
    WaitingHuman,
    IdleBetweenTurns,
    Ended,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionReachability {
    #[default]
    Active,
    Idle,
    Unconfirmed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PendingRequestKind {
    Approval,
    #[default]
    Question,
    ManualWait,
}

/// Persistent pending human decision (approval prompt or explicit question).
/// Decoupled from liveness/heartbeat timers: NEVER expires silently after 2 minutes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingHumanRequest {
    pub request_id: String,
    #[serde(default)]
    pub kind: PendingRequestKind,
    pub reason: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub source: TelemetrySource,
    #[serde(default)]
    pub tool_name: Option<String>,
}

/// Per-session lease stored independently in `<runtime_dir>/<session_id>.json`.
/// Separates task semantics (`issue_id`, `scope`), persistent human wait (`pending_request`),
/// turn lifecycle (`turn_state`), and honest reachability (`reachability`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLease {
    pub session_id: String,
    #[serde(default)]
    pub issue_id: Option<String>,
    #[serde(default)]
    pub worktree_id: String,
    #[serde(default)]
    pub worktree_path: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub harness: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub source: TelemetrySource,
    #[serde(default)]
    pub state: SessionState,
    #[serde(default)]
    pub turn_state: TurnState,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub waiting_reason: Option<String>,
    #[serde(default)]
    pub pending_request: Option<PendingHumanRequest>,
    #[serde(default)]
    pub last_tool: Option<String>,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub thread_url: Option<String>,
    #[serde(default)]
    pub seq: u64,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub last_seen_at: String,
    #[serde(default)]
    pub last_activity_at: String,
    #[serde(default)]
    pub last_progress_at: String,
    #[serde(default)]
    pub expires_at: String,
    /// Computed at read time from `last_activity_at` / `turn_state`.
    #[serde(default)]
    pub reachability: SessionReachability,
    /// Computed compatibility flag (`reachability == Unconfirmed`).
    #[serde(default)]
    pub stale_heartbeat: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanStatus {
    #[default]
    Ok,
    Partial,
    Stale,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionSource {
    ExplicitLease,
    BranchIssueHint,
    #[default]
    Unattributed,
}

/// Passively observed local Git worktree state (dirty working tree + committed branch diff vs base).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeActivity {
    pub worktree_id: String,
    pub path: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub is_main: bool,
    pub is_bare: bool,
    pub is_locked: bool,
    pub is_prunable: bool,
    pub scan_status: ScanStatus,
    pub scan_error: Option<String>,
    pub observed_at_ms: u64,
    pub modified_count: usize,
    pub untracked_count: usize,
    /// Bounded sample of uncommitted dirty paths for lean IPC payloads.
    pub dirty_sample: Vec<String>,
    pub truncated: bool,
    /// Paths committed on this branch since `merge-base(base_ref, HEAD)`.
    #[serde(default)]
    pub committed_count: usize,
    #[serde(default)]
    pub committed_sample: Vec<String>,
    #[serde(default)]
    pub committed_truncated: bool,
    #[serde(default)]
    pub base_ref: Option<String>,
    #[serde(default)]
    pub branch_diff_status: ScanStatus,
    pub attribution: AttributionSource,
    pub session_ids: Vec<String>,
    pub linked_issue_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParticipantKind {
    Session,
    Worktree,
    Issue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantRef {
    pub kind: ParticipantKind,
    pub id: String,
    pub label: String,
    pub issue_id: Option<String>,
    pub worktree_id: Option<String>,
    pub branch: Option<String>,
    pub harness: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    /// Strongest: two distinct worktrees both have live dirty edits to the exact same path.
    SameFileDirty,
    /// Strong: two distinct worktrees touched the exact same path across committed (`merge-base..HEAD`) + dirty edits.
    SameFileCommitted,
    /// High-risk setup: 2+ active sessions share the exact same checkout directory (Git cannot separate edits).
    SharedWorktree,
    /// Medium: one worktree's dirty or committed path hits another participant's declared write scope.
    DirtyHitsDeclaredScope,
    /// Advisory: two active sessions/issues declare overlapping write scopes.
    DeclaredScopeOverlap,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEvidence {
    pub paths: Vec<String>,
    pub matched_scopes: Vec<String>,
    pub total_paths: usize,
    pub lockfile_only: bool,
    #[serde(default)]
    pub base_ref: Option<String>,
    #[serde(default)]
    pub evidence_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictAlert {
    pub id: String,
    pub kind: ConflictKind,
    pub participants: Vec<ParticipantRef>,
    pub evidence: ConflictEvidence,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockerHotspot {
    pub issue_id: String,
    pub short_id: String,
    pub title: String,
    pub lane: BoardLane,
    pub actionable: bool,
    pub direct_unblocks: usize,
    pub total_downstream: usize,
    pub downstream_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttentionKind {
    WaitingHuman,
    FileOverlap,
    SharedWorktree,
    ProgressStale,
    HeartbeatStale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionItem {
    pub id: String,
    pub kind: AttentionKind,
    pub issue_id: Option<String>,
    pub session_id: Option<String>,
    pub conflict_id: Option<String>,
    pub harness: Option<String>,
    pub agent: Option<String>,
    pub reason: String,
    pub age_minutes: u64,
    pub thread_url: Option<String>,
    /// True when the pending request is still shown from last-known state while host reachability is unconfirmed.
    #[serde(default)]
    pub unconfirmed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarCoverage {
    pub scope_note: String,
    #[serde(default)]
    pub base_ref: Option<String>,
    pub scanned_worktrees: usize,
    pub total_worktrees: usize,
    pub failed_worktrees: usize,
    pub scan_status: ScanStatus,
    pub observed_at_ms: u64,
}

/// Ephemeral runtime telemetry snapshot decoupled from `WorkspaceSnapshot`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarSnapshot {
    pub repo_id: String,
    pub repo_path: String,
    pub generation: u64,
    pub sessions: Vec<SessionLease>,
    pub worktrees: Vec<WorktreeActivity>,
    pub conflicts: Vec<ConflictAlert>,
    pub top_blockers: Vec<BlockerHotspot>,
    pub attention: Vec<AttentionItem>,
    pub coverage: RadarCoverage,
}

// ---------------------------------------------------------------------------
// Requests & Exchange Reports
// ---------------------------------------------------------------------------

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
    #[serde(default)]
    pub scope: Vec<String>,
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
    pub scope: Option<Vec<String>>,
    pub expected_rev: Option<u64>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnblockedSummary {
    pub id: String,
    pub short_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishResult {
    pub id: String,
    pub status: String,
    pub rev: u64,
    pub idempotent_replay: bool,
    pub unblocked: Vec<UnblockedSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub source_path: String,
    pub store_path: String,
    pub imported_count: usize,
    pub updated_count: usize,
    pub skipped_malformed_lines: Vec<usize>,
    pub duplicate_ids: Vec<String>,
    pub dangling_dependencies: Vec<String>,
    pub source_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub format: String,
    pub dest_path: String,
    pub exported_count: usize,
    pub exported_digest: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub ok: bool,
    pub version: String,
    pub repo_id: String,
    pub repo_name: String,
    pub worktree_root: String,
    pub worktree_id: String,
    pub branch: Option<String>,
    pub git_common_dir: Option<String>,
    pub store_dir: String,
    pub store_kind: StoreKind,
    pub storage_backend: StorageBackend,
    pub has_store: bool,
    pub runtime_dir: String,
    pub issue_count: usize,
    pub active_sessions: usize,
    pub warnings: Vec<String>,
}
