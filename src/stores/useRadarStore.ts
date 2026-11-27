import { create } from "zustand";
import {
  formatIpcError,
  ipcClearSessionWaiting,
  ipcGetRadarSnapshot,
  ipcReleaseSession,
} from "@/lib/ipc";
import type { ConflictAlert, RadarSnapshot, SessionLease } from "@/types/ipc";

export interface IssueRuntimeBadge {
  kind: "waiting" | "conflict" | "working" | "stale";
  label: string;
  sessionCount: number;
}

interface RadarState {
  radar: RadarSnapshot | null;
  scanning: boolean;
  lastError: string | null;
  lastGitScanAt: number;
  /** Pre-indexed map so BoardCard subscribes cheaply by issueId without scanning arrays */
  badgeByIssueId: Record<string, IssueRuntimeBadge>;
  sessionsByIssueId: Record<string, SessionLease[]>;
  conflictsByIssueId: Record<string, ConflictAlert[]>;

  applyRadar: (snap: RadarSnapshot) => void;
  clearRadar: () => void;
  refreshRadar: (workspacePath: string, scanGit?: boolean) => Promise<void>;
  clearSessionWaiting: (workspacePath: string, sessionId: string) => Promise<void>;
  releaseSession: (workspacePath: string, sessionId: string) => Promise<void>;
}

function buildIndices(snap: RadarSnapshot): {
  badgeByIssueId: Record<string, IssueRuntimeBadge>;
  sessionsByIssueId: Record<string, SessionLease[]>;
  conflictsByIssueId: Record<string, ConflictAlert[]>;
} {
  const sessionsByIssueId: Record<string, SessionLease[]> = {};
  for (const s of snap.sessions) {
    if (!s.issueId) continue;
    const arr = sessionsByIssueId[s.issueId] ?? [];
    arr.push(s);
    sessionsByIssueId[s.issueId] = arr;
  }

  const conflictsByIssueId: Record<string, ConflictAlert[]> = {};
  for (const c of snap.conflicts) {
    for (const p of c.participants) {
      if (!p.issueId) continue;
      const arr = conflictsByIssueId[p.issueId] ?? [];
      if (!arr.some((x) => x.id === c.id)) {
        arr.push(c);
      }
      conflictsByIssueId[p.issueId] = arr;
    }
  }

  // Pick at most ONE highest-priority runtime badge per issue:
  // waiting_human > file-overlap conflict > active working > stale heartbeat
  const badgeByIssueId: Record<string, IssueRuntimeBadge> = {};
  const allIssueIds = new Set([
    ...Object.keys(sessionsByIssueId),
    ...Object.keys(conflictsByIssueId),
  ]);

  for (const issueId of allIssueIds) {
    const sessions = sessionsByIssueId[issueId] ?? [];
    const conflicts = conflictsByIssueId[issueId] ?? [];
    const live = sessions.filter((s) => s.reachability !== "unconfirmed" && !s.staleHeartbeat);
    // Persistent pendingRequest takes top priority even if reachability became unconfirmed!
    const waiting = sessions.find((s) => Boolean(s.pendingRequest) || s.state === "waiting_human");
    const severeConflict = conflicts.find(
      (c) =>
        (c.kind === "same-file-dirty" || c.kind === "same-file-committed") &&
        !c.evidence.lockfileOnly,
    );

    if (waiting) {
      badgeByIssueId[issueId] = {
        kind: "waiting",
        label: waiting.harness,
        sessionCount: Math.max(live.length, 1),
      };
    } else if (severeConflict) {
      badgeByIssueId[issueId] = {
        kind: "conflict",
        label: severeConflict.evidence.paths[0] ?? "",
        sessionCount: live.length,
      };
    } else if (live.length > 0 && live[0]) {
      badgeByIssueId[issueId] = {
        kind: "working",
        label: live.length > 1 ? `${live[0].harness} +${live.length - 1}` : live[0].harness,
        sessionCount: live.length,
      };
    } else if (sessions.length > 0 && sessions[0]) {
      badgeByIssueId[issueId] = {
        kind: "stale",
        label: sessions[0].harness,
        sessionCount: sessions.length,
      };
    }
  }

  return { badgeByIssueId, sessionsByIssueId, conflictsByIssueId };
}

export const useRadarStore = create<RadarState>((set, get) => ({
  radar: null,
  scanning: false,
  lastError: null,
  lastGitScanAt: 0,
  badgeByIssueId: {},
  sessionsByIssueId: {},
  conflictsByIssueId: {},

  applyRadar: (snap) => {
    const prev = get().radar;
    if (prev && prev.repoId === snap.repoId && snap.generation < prev.generation) {
      return;
    }
    const indices = buildIndices(snap);
    set({
      radar: snap,
      lastError: null,
      ...indices,
    });
  },

  clearRadar: () =>
    set({
      radar: null,
      scanning: false,
      lastError: null,
      badgeByIssueId: {},
      sessionsByIssueId: {},
      conflictsByIssueId: {},
    }),

  refreshRadar: async (workspacePath, scanGit = true) => {
    if (!workspacePath) return;
    set({ scanning: true, lastError: null });
    try {
      const snap = await ipcGetRadarSnapshot(workspacePath, scanGit);
      get().applyRadar(snap);
      if (scanGit) {
        set({ lastGitScanAt: Date.now() });
      }
    } catch (err) {
      set({ lastError: formatIpcError(err) });
    } finally {
      set({ scanning: false });
    }
  },

  clearSessionWaiting: async (workspacePath, sessionId) => {
    const snap = await ipcClearSessionWaiting(workspacePath, sessionId);
    get().applyRadar(snap);
  },

  releaseSession: async (workspacePath, sessionId) => {
    const snap = await ipcReleaseSession(workspacePath, sessionId);
    get().applyRadar(snap);
  },
}));
