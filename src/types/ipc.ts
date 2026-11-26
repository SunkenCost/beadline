export type Locale = "zh-CN" | "en";

export type ThemePresetId = "warm-claude" | "linear-slate" | "pure-mono";

export type AppearanceMode = "light" | "dark" | "system";

export type RadiusScale = "sharp" | "default" | "soft";

export type DensityMode = "compact" | "comfortable";

export type BoardLane = "in_progress" | "ready" | "blocked" | "closed";

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

export interface IssueDto {
  id: string;
  shortId: string;
  title: string;
  description: string;
  notes: string;
  acceptanceCriteria: string;
  closeReason: string;
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
  repoName: string;
  repoPath: string;
  hasBeads: boolean;
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

export interface ModuleAliasConfig {
  label?: string | null;
  color?: string | null;
}

export interface WorkspaceSummary {
  name: string;
  path: string;
  hasBeads: boolean;
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
}

export interface CreateIssueResponse {
  id: string;
  snapshot: WorkspaceSnapshot;
}

export interface GithubSyncResponse {
  output: string;
  snapshot: WorkspaceSnapshot;
}
