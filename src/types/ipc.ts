export type Locale = "zh-CN" | "en";

export type ThemePresetId = "warm-clay" | "cool-slate" | "pure-mono";

export type AppearanceMode = "light" | "dark" | "system";

export type RadiusScale = "sharp" | "default" | "soft";

export type DensityMode = "compact" | "comfortable";

export type BoardLane = "in_progress" | "ready" | "blocked" | "closed";

export type StorageBackend = "native-jsonl" | "dolt-cli";

export type StoreKind = "shared-git-common" | "legacy-beads-dir" | "local-beads-dir";

export interface ManagedBdStatus {
  installed: boolean;
  path: string | null;
  source: string | null;
  version: string | null;
}

export interface BlCliStatus {
  installed: boolean;
  path: string | null;
  version: string | null;
}

export type IssueStatus = "open" | "in_progress" | "blocked" | "closed";

export type IssueType =
  | "epic"
  | "task"
  | "bug"
  | "feature"
  | "chore"
  | "decision"
  | "spike"
  | "story"
  | "milestone";

export interface BeadlineThemeV1 {
  schemaVersion: 1;
  id: string;
  name: string;
  appearance: AppearanceMode;
  tokens: Record<string, string>;
}

export interface ThemeSettings {
  preset: ThemePresetId;
  appearance: AppearanceMode;
  accentOverride: string | null;
  radius: RadiusScale;
  density: DensityMode;
  activeCustomThemeId: string | null;
  customThemes: BeadlineThemeV1[];
}

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
  maximized: boolean;
}

export interface AppSettings {
  language: Locale;
  theme: ThemeSettings;
  bdPath: string | null;
  useForceWrites: boolean;
  deepLinkSchemes: string[];
  window: WindowGeometry | null;
}

export interface PlatformInfo {
  os: string;
  arch: string;
  version: string;
}

export interface AppErrorDto {
  code: string;
  message: string;
  detail?: string | null;
}

export interface SmartLink {
  scheme: string;
  url: string;
  label: string;
}

export interface WorktreeRef {
  path: string;
  branch: string;
}

export interface SmartMeta {
  githubNumber: number | null;
  githubUrl: string | null;
  branches: string[];
  commits: string[];
  worktrees: WorktreeRef[];
  links: SmartLink[];
}

export interface IssueProof {
  summary: string;
  refs: string[];
  operationId: string | null;
  submittedAt: string;
  sessionId: string | null;
  actor: string | null;
}

export interface IssueDto {
  id: string;
  shortId: string;
  title: string;
  description: string;
  notes: string;
  acceptanceCriteria: string;
  closeReason: string;
  scope?: string[];
  proof?: IssueProof | null;
  rev?: number;
  status: string;
  lane: BoardLane;
  priority: number;
  issueType: string;
  assignee: string;
  owner: string;
  createdAt: string;
  updatedAt: string;
  closedAt: string;
  externalRef: string;
  labels: string[];
  modules: string[];
  parentId: string | null;
  parentShortId: string | null;
  parentTitle: string | null;
  childrenIds: string[];
  blockedBy: string[];
  openBlockedBy: string[];
  blocks: string[];
  commentCount: number;
  smart: SmartMeta;
}

export interface DagEdge {
  from: string;
  to: string;
}

export interface ModuleCount {
  key: string;
  count: number;
}

export interface BoardStats {
  total: number;
  inProgress: number;
  ready: number;
  blocked: number;
  closed: number;
  epics: number;
}

export interface WorkspaceSnapshot {
  repoId?: string;
  repoName: string;
  repoPath: string;
  storePath?: string;
  storeKind?: StoreKind;
  hasBeads: boolean;
  storageBackend: StorageBackend;
  prefix: string | null;
  githubRepo: string | null;
  modulePrefix: string;
  fingerprint: string;
  stats: BoardStats;
  modules: ModuleCount[];
  issues: IssueDto[];
  epics: IssueDto[];
  edges: DagEdge[];
}

export interface CommentDto {
  id: string;
  author: string;
  text: string;
  createdAt: string;
}

export type SessionState = "working" | "waiting_human" | "released";

export type TelemetrySource = "hook" | "mcp" | "cli";

export type TurnState = "running" | "waiting-human" | "idle-between-turns" | "ended";

export type SessionReachability = "active" | "idle" | "unconfirmed";

export type PendingRequestKind = "approval" | "question" | "manual-wait";

export interface PendingHumanRequest {
  requestId: string;
  kind: PendingRequestKind;
  reason: string;
  createdAt: string;
  source: TelemetrySource;
  toolName: string | null;
}

export interface SessionLease {
  sessionId: string;
  issueId: string | null;
  worktreeId: string;
  worktreePath: string;
  branch: string | null;
  harness: string;
  agent: string;
  source?: TelemetrySource;
  state: SessionState;
  turnState?: TurnState;
  summary: string;
  waitingReason: string | null;
  pendingRequest?: PendingHumanRequest | null;
  lastTool?: string | null;
  scope: string[];
  threadUrl: string | null;
  seq: number;
  startedAt: string;
  lastSeenAt: string;
  lastActivityAt?: string;
  lastProgressAt: string;
  expiresAt: string;
  reachability?: SessionReachability;
  staleHeartbeat: boolean;
}

export type ScanStatus = "ok" | "partial" | "stale" | "failed" | "timeout";

export type AttributionSource = "explicit-lease" | "branch-issue-hint" | "unattributed";

export interface WorktreeActivity {
  worktreeId: string;
  path: string;
  branch: string | null;
  head: string | null;
  isMain: boolean;
  isBare: boolean;
  isLocked: boolean;
  isPrunable: boolean;
  scanStatus: ScanStatus;
  scanError: string | null;
  observedAtMs: number;
  modifiedCount: number;
  untrackedCount: number;
  dirtySample: string[];
  truncated: boolean;
  committedCount?: number;
  committedSample?: string[];
  committedTruncated?: boolean;
  baseRef?: string | null;
  branchDiffStatus?: ScanStatus;
  attribution: AttributionSource;
  sessionIds: string[];
  linkedIssueIds: string[];
}

export type ParticipantKind = "session" | "worktree" | "issue";

export interface ParticipantRef {
  kind: ParticipantKind;
  id: string;
  label: string;
  issueId: string | null;
  worktreeId: string | null;
  branch: string | null;
  harness: string | null;
}

export type ConflictKind =
  | "same-file-dirty"
  | "same-file-committed"
  | "shared-worktree"
  | "dirty-hits-declared-scope"
  | "declared-scope-overlap";

export interface ConflictEvidence {
  paths: string[];
  matchedScopes: string[];
  totalPaths: number;
  lockfileOnly: boolean;
  baseRef?: string | null;
  evidenceNote?: string | null;
}

export interface ConflictAlert {
  id: string;
  kind: ConflictKind;
  participants: ParticipantRef[];
  evidence: ConflictEvidence;
  observedAtMs: number;
}

export interface BlockerHotspot {
  issueId: string;
  shortId: string;
  title: string;
  lane: BoardLane;
  actionable: boolean;
  directUnblocks: number;
  totalDownstream: number;
  downstreamIds: string[];
}

export type AttentionKind =
  | "waiting-human"
  | "file-overlap"
  | "shared-worktree"
  | "progress-stale"
  | "heartbeat-stale";

export interface AttentionItem {
  id: string;
  kind: AttentionKind;
  issueId: string | null;
  sessionId: string | null;
  conflictId: string | null;
  harness: string | null;
  agent: string | null;
  reason: string;
  ageMinutes: number;
  threadUrl: string | null;
  unconfirmed?: boolean;
}

export interface RadarCoverage {
  scopeNote: string;
  baseRef?: string | null;
  scannedWorktrees: number;
  totalWorktrees: number;
  failedWorktrees: number;
  scanStatus: ScanStatus;
  observedAtMs: number;
}

export interface RadarSnapshot {
  repoId: string;
  repoPath: string;
  generation: number;
  sessions: SessionLease[];
  worktrees: WorktreeActivity[];
  conflicts: ConflictAlert[];
  topBlockers: BlockerHotspot[];
  attention: AttentionItem[];
  coverage: RadarCoverage;
}

export interface DoctorReport {
  ok: boolean;
  version: string;
  repoId: string;
  repoName: string;
  worktreeRoot: string;
  worktreeId: string;
  branch: string | null;
  gitCommonDir: string | null;
  storeDir: string;
  storeKind: StoreKind;
  storageBackend: StorageBackend;
  hasStore: boolean;
  runtimeDir: string;
  issueCount: number;
  activeSessions: number;
  warnings: string[];
}

export interface ModuleAliasConfig {
  label?: string | null;
  color?: string | null;
}

export interface WorkspaceSummary {
  name: string;
  path: string;
  hasBeads: boolean;
  storageBackend: StorageBackend;
  prefix: string | null;
  issueCount: number;
  inProgressCount: number;
  isRegistered: boolean;
  pinned: boolean;
  modulePrefix: string;
  moduleAliases: Record<string, ModuleAliasConfig>;
}

export interface WorkspaceListResponse {
  activePath: string | null;
  workspaces: WorkspaceSummary[];
}

export interface CrossProjectActiveItem {
  workspaceName: string;
  workspacePath: string;
  issue: IssueDto;
}

export interface CreateIssueRequest {
  workspacePath: string;
  title: string;
  issueType: string;
  priority: number;
  status: string;
  assignee?: string;
  parentId?: string;
  labels: string[];
  description?: string;
  acceptanceCriteria?: string;
  notes?: string;
  scope?: string[];
  blockedBy: string[];
}

export interface UpdateIssueRequest {
  workspacePath: string;
  issueId: string;
  title?: string;
  status?: string;
  priority?: number;
  assignee?: string;
  labels?: string[];
  description?: string;
  acceptanceCriteria?: string;
  notes?: string;
  scope?: string[];
  expectedRev?: number;
}

export interface CreateIssueResponse {
  id: string;
  snapshot: WorkspaceSnapshot;
}

export interface GithubSyncResponse {
  output: string;
  snapshot: WorkspaceSnapshot;
}
