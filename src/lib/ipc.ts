import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  AppErrorDto,
  AppSettings,
  CommentDto,
  CreateIssueRequest,
  CreateIssueResponse,
  CrossProjectActiveItem,
  GithubSyncResponse,
  ModuleAliasConfig,
  PlatformInfo,
  UpdateIssueRequest,
  WorkspaceListResponse,
  WorkspaceSnapshot,
} from "@/types/ipc";

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function formatIpcError(err: unknown): string {
  if (err && typeof err === "object") {
    const dto = err as Partial<AppErrorDto>;
    if (typeof dto.message === "string") {
      return dto.detail ? `${dto.message}: ${dto.detail}` : dto.message;
    }
  }
  if (err instanceof Error) return err.message;
  return String(err);
}

export const DEFAULT_SETTINGS: AppSettings = {
  language: "zh-CN",
  theme: {
    preset: "warm-claude",
    appearance: "system",
    accentOverride: null,
    radius: "default",
    density: "comfortable",
    activeCustomThemeId: null,
    customThemes: [],
  },
  bdPath: null,
  useForceWrites: true,
  deepLinkSchemes: ["t3-thread", "vscode", "cursor"],
  window: null,
};

export async function ipcGetSettings(): Promise<AppSettings> {
  if (!isTauriRuntime()) return DEFAULT_SETTINGS;
  return invoke<AppSettings>("get_settings");
}

export async function ipcUpdateSettings(settings: AppSettings): Promise<AppSettings> {
  if (!isTauriRuntime()) return settings;
  return invoke<AppSettings>("update_settings", { settings });
}

export async function ipcGetPlatformInfo(): Promise<PlatformInfo> {
  if (!isTauriRuntime()) return { os: "web", arch: "browser", version: "0.1.0" };
  return invoke<PlatformInfo>("get_platform_info");
}

export async function ipcListWorkspaces(): Promise<WorkspaceListResponse> {
  if (!isTauriRuntime()) return { activePath: null, workspaces: [] };
  return invoke<WorkspaceListResponse>("list_workspaces");
}

export async function ipcAddWorkspace(path: string): Promise<WorkspaceListResponse> {
  return invoke<WorkspaceListResponse>("add_workspace", { path });
}

export async function ipcSelectWorkspace(path: string): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("select_workspace", { path });
}

export async function ipcRemoveWorkspace(path: string): Promise<WorkspaceListResponse> {
  return invoke<WorkspaceListResponse>("remove_workspace", { path });
}

export async function ipcUpdateWorkspaceConfig(params: {
  path: string;
  modulePrefix: string;
  moduleAliases: Record<string, ModuleAliasConfig>;
  pinned: boolean;
}): Promise<WorkspaceListResponse> {
  return invoke<WorkspaceListResponse>("update_workspace_config", params);
}

export async function ipcInitBeads(path: string, prefix?: string): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("init_beads", { path, prefix: prefix ?? null });
}

export async function ipcGetSnapshot(
  workspacePath: string,
  force = false,
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("get_snapshot", { workspacePath, force });
}

export async function ipcGetAllActive(): Promise<CrossProjectActiveItem[]> {
  if (!isTauriRuntime()) return [];
  return invoke<CrossProjectActiveItem[]>("get_all_active");
}

export async function ipcGetIssueComments(
  workspacePath: string,
  issueId: string,
): Promise<CommentDto[]> {
  return invoke<CommentDto[]>("get_issue_comments", { workspacePath, issueId });
}

export async function ipcCreateIssue(request: CreateIssueRequest): Promise<CreateIssueResponse> {
  return invoke<CreateIssueResponse>("create_issue", { request });
}

export async function ipcUpdateIssue(request: UpdateIssueRequest): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("update_issue", { request });
}

export async function ipcCloseIssue(
  workspacePath: string,
  issueId: string,
  reason?: string,
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("close_issue", {
    workspacePath,
    issueId,
    reason: reason ?? null,
  });
}

export async function ipcReopenIssue(
  workspacePath: string,
  issueId: string,
  reason?: string,
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("reopen_issue", {
    workspacePath,
    issueId,
    reason: reason ?? null,
  });
}

export async function ipcAddDependency(
  workspacePath: string,
  blockedId: string,
  blockerId: string,
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("add_dependency", {
    workspacePath,
    blockedId,
    blockerId,
  });
}

export async function ipcRemoveDependency(
  workspacePath: string,
  blockedId: string,
  blockerId: string,
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("remove_dependency", {
    workspacePath,
    blockedId,
    blockerId,
  });
}

export async function ipcAddComment(
  workspacePath: string,
  issueId: string,
  text: string,
): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("add_comment", { workspacePath, issueId, text });
}

export async function ipcGithubSync(
  workspacePath: string,
  pullOnly = true,
): Promise<GithubSyncResponse> {
  return invoke<GithubSyncResponse>("github_sync", { workspacePath, pullOnly });
}

export async function ipcOpenExternalLink(url: string): Promise<void> {
  if (!isTauriRuntime()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  await invoke<void>("open_external_link", { url });
}

export async function ipcRevealPath(path: string): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke<void>("reveal_path", { path });
}

export async function ipcDetectBdPath(): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  return invoke<string | null>("detect_bd_path");
}

export async function ipcPickFolder(title: string): Promise<string | null> {
  if (!isTauriRuntime()) return null;
  const res = await openDialog({
    directory: true,
    multiple: false,
    title,
  });
  return typeof res === "string" ? res : null;
}

export async function subscribeSnapshotUpdated(
  handler: (snapshot: WorkspaceSnapshot) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) return () => undefined;
  return listen<WorkspaceSnapshot>("beadline://snapshot-updated", (ev) => {
    handler(ev.payload);
  });
}
