import { describe, expect, it } from "vitest";
import type { IssueDto } from "@/types/ipc";
import { extractPlainSummary, isClosedIterationRound, shortParentLabel } from "./human-context";

function mockIssue(overrides: Partial<IssueDto>): IssueDto {
  return {
    id: "pbl-1",
    shortId: "pbl-1",
    title: "Task",
    description: "",
    notes: "",
    acceptanceCriteria: "",
    closeReason: "",
    status: "closed",
    lane: "closed",
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

describe("human-context helpers", () => {
  it("extracts a clean plain-language summary from markdown", () => {
    const md =
      "## 背景与现状\n\n1. **300+ 条开放事实沉睡在账本中**：在九册 `v3.7.1` 阶段需要整理。";
    expect(extractPlainSummary(md, 60)).toBe(
      "300+ 条开放事实沉睡在账本中：在九册 v3.7.1 阶段需要整理。",
    );
  });

  it("strips bracket tags when forming short parent labels", () => {
    expect(
      shortParentLabel("[Release v3.7.1-p5] 九册三合一权威发布图包与最终主干合流收口", 12),
    ).toBe("九册三合一权威发布图包…");
    expect(shortParentLabel("Milestone 1: Tauri v2 核心骨架", 20)).toBe("Tauri v2 核心骨架");
  });

  it("detects closed AI iteration-round noise without hiding active tasks or epics", () => {
    expect(
      isClosedIterationRound(
        mockIssue({
          title: "Wave 12 r5 微修：补齐字段级目标类型校验",
          lane: "closed",
          issueType: "bug",
        }),
      ),
    ).toBe(true);

    expect(
      isClosedIterationRound(
        mockIssue({
          title: "Wave 12 r5 微修：补齐字段级目标类型校验",
          lane: "in_progress",
        }),
      ),
    ).toBe(false);

    expect(
      isClosedIterationRound(
        mockIssue({
          title: "实现 Rust 混合 Beads 数据引擎",
          lane: "closed",
        }),
      ),
    ).toBe(false);
  });
});
