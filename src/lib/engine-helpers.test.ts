import { describe, expect, it } from "vitest";
import type { IssueDto, WorkspaceSnapshot } from "@/types/ipc";
import { computeDagLayout } from "./dag-layout";
import { resolveModule } from "./modules";
import { buildAgentPrompt } from "./prompt-builder";

function mockIssue(id: string, overrides: Partial<IssueDto> = {}): IssueDto {
  return {
    id,
    shortId: id,
    title: `Task ${id}`,
    description: "Details",
    notes: "",
    acceptanceCriteria: "All checks pass",
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

describe("dag-layout, modules, and prompt-builder", () => {
  it("computes layered DAG positions respecting topological order", () => {
    const a = mockIssue("bl-1", { lane: "closed", status: "closed" });
    const b = mockIssue("bl-2", { lane: "ready", blockedBy: ["bl-1"] });
    const c = mockIssue("bl-3", { lane: "blocked", openBlockedBy: ["bl-2"], blockedBy: ["bl-2"] });

    const layout = computeDagLayout(
      [a, b, c],
      [
        { from: "bl-1", to: "bl-2" },
        { from: "bl-2", to: "bl-3" },
      ],
      "connected",
    );

    expect(layout.nodes).toHaveLength(3);
    const byId = Object.fromEntries(layout.nodes.map((n) => [n.issue.id, n]));
    expect(byId["bl-1"]?.layer).toBe(0);
    expect(byId["bl-2"]?.layer).toBe(1);
    expect(byId["bl-3"]?.layer).toBe(2);
    expect((byId["bl-2"]?.x ?? 0) > (byId["bl-1"]?.x ?? 0)).toBe(true);
  });

  it("resolves custom module aliases and fallback colors", () => {
    const built = resolveModule("mod:engine", "mod:", "zh-CN");
    expect(built.label).toBe("底层引擎");

    const custom = resolveModule("area:auth", "area:", "en", {
      "area:auth": { label: "Authentication", color: "#123456" },
    });
    expect(custom.label).toBe("Authentication");
    expect(custom.color).toBe("#123456");
  });

  it("builds a structured agent prompt with blockers and bd close commands", () => {
    const blocker = mockIssue("bl-1", { title: "Core Engine", status: "in_progress" });
    const issue = mockIssue("bl-2", {
      title: "Kanban Board",
      openBlockedBy: ["bl-1"],
      smart: {
        githubNumber: 12,
        githubUrl: "https://github.com/org/repo/issues/12",
        branches: ["feat/board"],
        commits: ["abc1234"],
        worktrees: [],
        links: [],
      },
    });
    const snap: WorkspaceSnapshot = {
      repoName: "beadline",
      repoPath: "Y:\\GitHub\\beadline",
      hasBeads: true,
      prefix: "bl",
      githubRepo: "org/repo",
      modulePrefix: "mod:",
      fingerprint: "fp",
      stats: { total: 2, inProgress: 1, ready: 0, blocked: 1, closed: 0, epics: 0 },
      modules: [],
      issues: [blocker, issue],
      epics: [],
      edges: [],
    };
    const prompt = buildAgentPrompt(issue, snap, { "bl-1": blocker, "bl-2": issue }, "en");
    expect(prompt).toContain("Task Brief: bl-2");
    expect(prompt).toContain("feat/board");
    expect(prompt).toContain("Core Engine");
    expect(prompt).toContain("bd close bl-2 --force");
  });
});
