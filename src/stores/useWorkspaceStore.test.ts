import { beforeEach, describe, expect, it } from "vitest";
import type { CrossProjectActiveItem, IssueDto, WorkspaceSnapshot } from "@/types/ipc";
import { useWorkspaceStore } from "./useWorkspaceStore";

function makeIssue(id: string, overrides: Partial<IssueDto> = {}): IssueDto {
  return {
    id,
    shortId: id,
    title: id,
    description: "",
    notes: "",
    acceptanceCriteria: "",
    closeReason: "",
    status: "in_progress",
    lane: "in_progress",
    priority: 1,
    issueType: "task",
    assignee: "",
    owner: "",
    createdAt: "",
    updatedAt: "2026-11-26T08:00:00Z",
    closedAt: "",
    externalRef: "",
    labels: [],
    modules: [],
    parentId: null,
    parentShortId: null,
    parentTitle: null,
    childrenIds: [],
    blockedBy: [],
    openBlockedBy: [],
    blocks: [],
    commentCount: 0,
    smart: {
      githubNumber: null,
      githubUrl: null,
      branches: [],
      commits: [],
      worktrees: [],
      links: [],
    },
    ...overrides,
  };
}

function makeSnapshot(fingerprint: string): WorkspaceSnapshot {
  const issue = makeIssue("bl-1");
  return {
    repoName: "beadline",
    repoPath: "Y:\\GitHub\\beadline",
    hasBeads: true,
    prefix: "bl",
    githubRepo: null,
    modulePrefix: "mod:",
    fingerprint,
    stats: { total: 1, inProgress: 1, ready: 0, blocked: 0, closed: 0, epics: 0 },
    modules: [],
    issues: [issue],
    epics: [],
    edges: [],
  };
}

describe("useWorkspaceStore deduplication", () => {
  beforeEach(() => {
    useWorkspaceStore.setState({
      workspaces: [],
      activePath: "Y:\\GitHub\\beadline",
      snapshot: null,
      byId: {},
      allActive: [],
      syncStatus: "idle",
      lastError: null,
      lastSyncedAt: null,
      loading: false,
    });
  });

  it("ignores duplicate snapshots with the same fingerprint without replacing object references", () => {
    const first = makeSnapshot("fp-aaa");
    expect(useWorkspaceStore.getState().applySnapshot(first)).toBe(true);
    const refAfterFirst = useWorkspaceStore.getState().byId["bl-1"];

    // Same fingerprint from a spurious watcher event must NOT replace state
    const duplicate = makeSnapshot("fp-aaa");
    expect(useWorkspaceStore.getState().applySnapshot(duplicate)).toBe(false);
    expect(useWorkspaceStore.getState().byId["bl-1"]).toBe(refAfterFirst);

    // Real fingerprint change must update state
    const updated = makeSnapshot("fp-bbb");
    expect(useWorkspaceStore.getState().applySnapshot(updated)).toBe(true);
    expect(useWorkspaceStore.getState().snapshot?.fingerprint).toBe("fp-bbb");
  });

  it("ignores duplicate allActive lists without triggering store updates", () => {
    const listA: CrossProjectActiveItem[] = [
      {
        workspaceName: "beadline",
        workspacePath: "Y:\\GitHub\\beadline",
        issue: makeIssue("bl-1"),
      },
    ];
    expect(useWorkspaceStore.getState().applyAllActive(listA)).toBe(true);
    const ref = useWorkspaceStore.getState().allActive;

    const listSame: CrossProjectActiveItem[] = [
      {
        workspaceName: "beadline",
        workspacePath: "Y:\\GitHub\\beadline",
        issue: makeIssue("bl-1"),
      },
    ];
    expect(useWorkspaceStore.getState().applyAllActive(listSame)).toBe(false);
    expect(useWorkspaceStore.getState().allActive).toBe(ref);

    const listChanged: CrossProjectActiveItem[] = [
      {
        workspaceName: "beadline",
        workspacePath: "Y:\\GitHub\\beadline",
        issue: makeIssue("bl-1", { updatedAt: "2026-11-26T08:05:00Z" }),
      },
    ];
    expect(useWorkspaceStore.getState().applyAllActive(listChanged)).toBe(true);
  });
});
