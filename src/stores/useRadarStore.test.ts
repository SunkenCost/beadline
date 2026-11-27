import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RadarSnapshot } from "@/types/ipc";

const { mockGetRadarSnapshot } = vi.hoisted(() => ({
  mockGetRadarSnapshot: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  formatIpcError: (err: unknown) => String(err),
  ipcGetRadarSnapshot: mockGetRadarSnapshot,
  ipcClearSessionWaiting: vi.fn(),
  ipcReleaseSession: vi.fn(),
}));

import { useRadarStore } from "./useRadarStore";

function makeSnapshot(
  repoPath: string,
  repoId: string,
  generation: number,
  issueId: string,
): RadarSnapshot {
  return {
    repoId,
    repoPath,
    generation,
    sessions: [
      {
        sessionId: `ses_${repoId}`,
        issueId,
        worktreeId: `wt_${repoId}`,
        worktreePath: repoPath,
        branch: "main",
        harness: "codex",
        agent: "gpt",
        source: "hook",
        state: "working",
        turnState: "running",
        summary: `Working in ${repoId}`,
        waitingReason: null,
        pendingRequest: null,
        lastTool: null,
        scope: [],
        threadUrl: null,
        seq: 1,
        startedAt: "2026-11-27T01:55:00Z",
        lastSeenAt: "2026-11-27T02:00:00Z",
        lastActivityAt: "2026-11-27T02:00:00Z",
        lastProgressAt: "2026-11-27T02:00:00Z",
        expiresAt: "2026-11-27T02:05:00Z",
        reachability: "active",
        staleHeartbeat: false,
      },
    ],
    worktrees: [],
    conflicts: [],
    topBlockers: [],
    attention: [],
    coverage: {
      scopeNote: "git status + branch diff",
      baseRef: "main",
      totalWorktrees: 1,
      scannedWorktrees: 1,
      failedWorktrees: 0,
      scanStatus: "ok",
      observedAtMs: 1_764_208_800_000,
    },
  };
}

function deferred<T>() {
  let resolve!: (val: T) => void;
  let reject!: (err: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("useRadarStore workspace switching race protection", () => {
  beforeEach(() => {
    mockGetRadarSnapshot.mockReset();
    useRadarStore.getState().clearRadar();
  });

  it("prevents a late response from workspace A from overwriting workspace B", async () => {
    const defA = deferred<RadarSnapshot>();
    const defB = deferred<RadarSnapshot>();

    mockGetRadarSnapshot.mockImplementation((workspacePath: string) => {
      if (workspacePath === "Y:\\Work\\repo-a") return defA.promise;
      if (workspacePath === "Y:\\Work\\repo-b") return defB.promise;
      throw new Error(`unexpected path: ${workspacePath}`);
    });

    const pA = useRadarStore.getState().refreshRadar("Y:\\Work\\repo-a", true);
    const pB = useRadarStore.getState().refreshRadar("Y:\\Work\\repo-b", true);

    // Workspace B resolves first
    defB.resolve(makeSnapshot("Y:\\Work\\repo-b", "repo_b", 1, "b-1"));
    await pB;
    expect(useRadarStore.getState().radar?.repoPath).toBe("Y:\\Work\\repo-b");
    expect(useRadarStore.getState().badgeByIssueId["b-1"]?.kind).toBe("working");

    // Workspace A resolves late (even with a higher generation number!)
    defA.resolve(makeSnapshot("Y:\\Work\\repo-a", "repo_a", 99, "a-1"));
    await pA;

    // Active radar and badges MUST stay on Workspace B
    expect(useRadarStore.getState().radar?.repoPath).toBe("Y:\\Work\\repo-b");
    expect(useRadarStore.getState().badgeByIssueId["b-1"]?.kind).toBe("working");
    expect(useRadarStore.getState().badgeByIssueId["a-1"]).toBeUndefined();
  });

  it("ignores late pushed radar events for a previously active workspace", () => {
    useRadarStore.getState().setActiveRepoPath("Y:\\Work\\repo-b");
    useRadarStore.getState().applyRadar(makeSnapshot("Y:\\Work\\repo-b", "repo_b", 2, "b-1"));
    useRadarStore.getState().applyRadar(makeSnapshot("Y:\\Work\\repo-a", "repo_a", 50, "a-1"));

    expect(useRadarStore.getState().radar?.repoPath).toBe("Y:\\Work\\repo-b");
    expect(useRadarStore.getState().badgeByIssueId["a-1"]).toBeUndefined();
  });

  it("ignores stale same-repo generations", () => {
    useRadarStore.getState().setActiveRepoPath("Y:\\Work\\repo-b");
    useRadarStore.getState().applyRadar(makeSnapshot("Y:\\Work\\repo-b", "repo_b", 5, "b-new"));
    useRadarStore.getState().applyRadar(makeSnapshot("Y:\\Work\\repo-b", "repo_b", 3, "b-old"));

    expect(useRadarStore.getState().radar?.generation).toBe(5);
    expect(useRadarStore.getState().badgeByIssueId["b-new"]).toBeTruthy();
    expect(useRadarStore.getState().badgeByIssueId["b-old"]).toBeUndefined();
  });
});
