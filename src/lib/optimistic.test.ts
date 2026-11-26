import { describe, expect, it } from "vitest";
import type { IssueDto, WorkspaceSnapshot } from "@/types/ipc";
import { applyOptimisticClose, applyOptimisticReopen, applyOptimisticUpdate } from "./optimistic";

function makeIssue(id: string, overrides: Partial<IssueDto> = {}): IssueDto {
  return {
    id,
    shortId: id,
    title: id,
    description: "",
    notes: "",
    acceptanceCriteria: "",
    closeReason: "",
    status: "open",
    lane: "ready",
    priority: 1,
    issueType: "task",
    assignee: "",
    owner: "",
    createdAt: "",
    updatedAt: "",
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

function makeSnapshot(): WorkspaceSnapshot {
  const a = makeIssue("bl-1", { status: "in_progress", lane: "in_progress", blocks: ["bl-2"] });
  const b = makeIssue("bl-2", {
    status: "open",
    lane: "blocked",
    blockedBy: ["bl-1"],
    openBlockedBy: ["bl-1"],
    labels: ["mod:ui"],
    modules: ["mod:ui"],
  });
  return {
    repoName: "beadline",
    repoPath: "Y:\\GitHub\\beadline",
    hasBeads: true,
    storageBackend: "native-jsonl",
    prefix: "bl",
    githubRepo: null,
    modulePrefix: "mod:",
    fingerprint: "fp1",
    stats: { total: 2, inProgress: 1, ready: 0, blocked: 1, closed: 0, epics: 0 },
    modules: [{ key: "mod:ui", count: 1 }],
    issues: [a, b],
    epics: [],
    edges: [{ from: "bl-1", to: "bl-2" }],
  };
}

describe("optimistic snapshot mutations", () => {
  it("unblocks downstream dependents immediately when closing a blocker", () => {
    const closed = applyOptimisticClose(makeSnapshot(), "bl-1", "Done");
    expect(closed.stats.closed).toBe(1);
    expect(closed.stats.ready).toBe(1);
    expect(closed.stats.blocked).toBe(0);
    const downstream = closed.issues.find((i) => i.id === "bl-2");
    expect(downstream?.lane).toBe("ready");
    expect(downstream?.openBlockedBy).toEqual([]);
  });

  it("re-blocks downstream dependents when reopening a blocker", () => {
    const closed = applyOptimisticClose(makeSnapshot(), "bl-1", "Done");
    const reopened = applyOptimisticReopen(closed, "bl-1");
    expect(reopened.stats.ready).toBe(1);
    expect(reopened.stats.blocked).toBe(1);
    const downstream = reopened.issues.find((i) => i.id === "bl-2");
    expect(downstream?.lane).toBe("blocked");
    expect(downstream?.openBlockedBy).toEqual(["bl-1"]);
  });

  it("recalculates modules and lane on optimistic update", () => {
    const updated = applyOptimisticUpdate(makeSnapshot(), {
      issueId: "bl-1",
      labels: ["mod:engine", "mod:ui"],
      status: "open",
    });
    expect(updated.stats.ready).toBe(1);
    expect(updated.modules.find((m) => m.key === "mod:ui")?.count).toBe(2);
    expect(updated.modules.find((m) => m.key === "mod:engine")?.count).toBe(1);
  });
});
