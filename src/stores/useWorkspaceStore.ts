import { create } from "zustand";
import {
  formatIpcError,
  ipcAddComment,
  ipcAddDependency,
  ipcAddWorkspace,
  ipcCloseIssue,
  ipcCreateIssue,
  ipcGetAllActive,
  ipcGetSnapshot,
  ipcGithubSync,
  ipcInitBeads,
  ipcListWorkspaces,
  ipcRemoveDependency,
  ipcRemoveWorkspace,
  ipcReopenIssue,
  ipcSelectWorkspace,
  ipcUpdateIssue,
  ipcUpdateWorkspaceConfig,
} from "@/lib/ipc";
import type {
  CreateIssueRequest,
  CrossProjectActiveItem,
  IssueDto,
  ModuleAliasConfig,
  UpdateIssueRequest,
  WorkspaceSnapshot,
  WorkspaceSummary,
} from "@/types/ipc";

export type SyncStatus = "idle" | "busy" | "error";

interface WorkspaceState {
  workspaces: WorkspaceSummary[];
  activePath: string | null;
  snapshot: WorkspaceSnapshot | null;
  byId: Record<string, IssueDto>;
  allActive: CrossProjectActiveItem[];
  syncStatus: SyncStatus;
  lastError: string | null;
  lastSyncedAt: number | null;
  loading: boolean;

  loadAll: () => Promise<void>;
  refreshWorkspaces: () => Promise<void>;
  refreshAllActive: () => Promise<void>;
  selectWorkspace: (path: string) => Promise<void>;
  addWorkspace: (path: string) => Promise<void>;
  removeWorkspace: (path: string) => Promise<void>;
  updateWorkspaceConfig: (params: {
    path: string;
    modulePrefix: string;
    moduleAliases: Record<string, ModuleAliasConfig>;
    pinned: boolean;
  }) => Promise<void>;
  initBeads: (path: string, prefix?: string) => Promise<void>;
  refreshSnapshot: (force?: boolean) => Promise<void>;
  applySnapshot: (snap: WorkspaceSnapshot) => void;

  createIssue: (req: Omit<CreateIssueRequest, "workspacePath">) => Promise<string | null>;
  updateIssue: (req: Omit<UpdateIssueRequest, "workspacePath">) => Promise<void>;
  closeIssue: (issueId: string, reason?: string) => Promise<void>;
  reopenIssue: (issueId: string, reason?: string) => Promise<void>;
  addDependency: (blockedId: string, blockerId: string) => Promise<void>;
  removeDependency: (blockedId: string, blockerId: string) => Promise<void>;
  addComment: (issueId: string, text: string) => Promise<void>;
  syncGithub: (pullOnly?: boolean) => Promise<string>;
}

function indexById(issues: IssueDto[]): Record<string, IssueDto> {
  const map: Record<string, IssueDto> = {};
  for (const it of issues) map[it.id] = it;
  return map;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  workspaces: [],
  activePath: null,
  snapshot: null,
  byId: {},
  allActive: [],
  syncStatus: "idle",
  lastError: null,
  lastSyncedAt: null,
  loading: false,

  applySnapshot: (snap) => {
    const active = get().activePath;
    if (active && snap.repoPath.toLowerCase() !== active.toLowerCase()) {
      return;
    }
    set((s) => ({
      snapshot: snap,
      byId: indexById(snap.issues),
      syncStatus: "idle",
      lastError: null,
      lastSyncedAt: Date.now(),
      workspaces: s.workspaces.map((w) =>
        w.path.toLowerCase() === snap.repoPath.toLowerCase()
          ? {
              ...w,
              hasBeads: snap.hasBeads,
              prefix: snap.prefix,
              issueCount: snap.stats.total,
              inProgressCount: snap.stats.inProgress,
            }
          : w,
      ),
    }));
  },

  loadAll: async () => {
    set({ loading: true, syncStatus: "busy" });
    try {
      const list = await ipcListWorkspaces();
      const active = list.activePath ?? list.workspaces[0]?.path ?? null;
      set({ workspaces: list.workspaces, activePath: active });

      if (active) {
        const snap = await ipcSelectWorkspace(active);
        get().applySnapshot(snap);
      } else {
        set({ snapshot: null, byId: {}, syncStatus: "idle" });
      }
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "error", lastError: formatIpcError(err) });
    } finally {
      set({ loading: false });
    }
  },

  refreshWorkspaces: async () => {
    try {
      const list = await ipcListWorkspaces();
      set({ workspaces: list.workspaces });
    } catch {
      // keep existing list
    }
  },

  refreshAllActive: async () => {
    try {
      const allActive = await ipcGetAllActive();
      set({ allActive });
    } catch {
      // ignore transient error
    }
  },

  selectWorkspace: async (path) => {
    set({ activePath: path, syncStatus: "busy", lastError: null });
    try {
      const snap = await ipcSelectWorkspace(path);
      if (get().activePath?.toLowerCase() === path.toLowerCase()) {
        get().applySnapshot(snap);
      }
      void get().refreshWorkspaces();
    } catch (err) {
      set({ syncStatus: "error", lastError: formatIpcError(err) });
      throw err;
    }
  },

  addWorkspace: async (path) => {
    set({ syncStatus: "busy", lastError: null });
    try {
      const list = await ipcAddWorkspace(path);
      const active = list.activePath ?? path;
      set({ workspaces: list.workspaces, activePath: active });
      const snap = await ipcSelectWorkspace(active);
      get().applySnapshot(snap);
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "error", lastError: formatIpcError(err) });
      throw err;
    }
  },

  removeWorkspace: async (path) => {
    const list = await ipcRemoveWorkspace(path);
    const nextActive = list.activePath;
    set({ workspaces: list.workspaces, activePath: nextActive });
    if (nextActive) {
      const snap = await ipcSelectWorkspace(nextActive);
      get().applySnapshot(snap);
    } else {
      set({ snapshot: null, byId: {}, syncStatus: "idle" });
    }
    void get().refreshAllActive();
  },

  updateWorkspaceConfig: async (params) => {
    const list = await ipcUpdateWorkspaceConfig(params);
    set({ workspaces: list.workspaces });
    if (get().activePath?.toLowerCase() === params.path.toLowerCase()) {
      await get().refreshSnapshot(false);
    }
  },

  initBeads: async (path, prefix) => {
    set({ syncStatus: "busy", lastError: null });
    try {
      const snap = await ipcInitBeads(path, prefix);
      set({ activePath: snap.repoPath });
      get().applySnapshot(snap);
      await get().refreshWorkspaces();
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "error", lastError: formatIpcError(err) });
      throw err;
    }
  },

  refreshSnapshot: async (force = false) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy", lastError: null });
    try {
      const snap = await ipcGetSnapshot(active, force);
      get().applySnapshot(snap);
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "error", lastError: formatIpcError(err) });
      throw err;
    }
  },

  createIssue: async (req) => {
    const active = get().activePath;
    if (!active) return null;
    set({ syncStatus: "busy" });
    try {
      const res = await ipcCreateIssue({ ...req, workspacePath: active });
      get().applySnapshot(res.snapshot);
      void get().refreshAllActive();
      return res.id;
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  updateIssue: async (req) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy" });
    try {
      const snap = await ipcUpdateIssue({ ...req, workspacePath: active });
      get().applySnapshot(snap);
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  closeIssue: async (issueId, reason) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy" });
    try {
      const snap = await ipcCloseIssue(active, issueId, reason);
      get().applySnapshot(snap);
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  reopenIssue: async (issueId, reason) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy" });
    try {
      const snap = await ipcReopenIssue(active, issueId, reason);
      get().applySnapshot(snap);
      void get().refreshAllActive();
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  addDependency: async (blockedId, blockerId) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy" });
    try {
      const snap = await ipcAddDependency(active, blockedId, blockerId);
      get().applySnapshot(snap);
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  removeDependency: async (blockedId, blockerId) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy" });
    try {
      const snap = await ipcRemoveDependency(active, blockedId, blockerId);
      get().applySnapshot(snap);
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  addComment: async (issueId, text) => {
    const active = get().activePath;
    if (!active) return;
    set({ syncStatus: "busy" });
    try {
      const snap = await ipcAddComment(active, issueId, text);
      get().applySnapshot(snap);
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },

  syncGithub: async (pullOnly = true) => {
    const active = get().activePath;
    if (!active) return "";
    set({ syncStatus: "busy" });
    try {
      const res = await ipcGithubSync(active, pullOnly);
      get().applySnapshot(res.snapshot);
      void get().refreshAllActive();
      return res.output;
    } catch (err) {
      set({ syncStatus: "idle", lastError: formatIpcError(err) });
      throw err;
    }
  },
}));
