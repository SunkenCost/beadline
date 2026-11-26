import type { IssueDto, Locale, WorkspaceSnapshot } from "@/types/ipc";

export function buildAgentPrompt(
  issue: IssueDto,
  snapshot: WorkspaceSnapshot,
  byId: Record<string, IssueDto>,
  locale: Locale,
): string {
  const isZh = locale === "zh-CN";
  const lines: string[] = [];

  lines.push(
    isZh
      ? `# 任务交接单：${issue.shortId} (${issue.id})`
      : `# Task Handoff: ${issue.shortId} (${issue.id})`,
  );
  lines.push("");
  lines.push(
    `- **${isZh ? "项目" : "Workspace"}**: ${snapshot.repoName} (\`${snapshot.repoPath}\`)`,
  );
  lines.push(`- **${isZh ? "标题" : "Title"}**: ${issue.title}`);
  lines.push(
    `- **${isZh ? "类型 / 优先级 / 状态" : "Type / Priority / Status"}**: ${issue.issueType} · P${issue.priority} · ${issue.status}${issue.rev ? ` · rev ${issue.rev}` : ""}`,
  );

  if (issue.parentTitle && issue.parentShortId) {
    lines.push(
      `- **${isZh ? "所属主目标" : "Parent Epic"}**: ${issue.parentShortId} — ${issue.parentTitle}`,
    );
  }
  if (issue.scope && issue.scope.length > 0) {
    lines.push(
      `- **${isZh ? "声明修改范围" : "Declared Write Scope"}**: ${issue.scope.map((s) => `\`${s}\``).join(", ")}`,
    );
  }
  if (issue.modules.length > 0) {
    lines.push(`- **${isZh ? "模块标签" : "Modules"}**: ${issue.modules.join(", ")}`);
  }
  if (issue.smart.branches.length > 0) {
    lines.push(
      `- **${isZh ? "关联分支" : "Branch"}**: ${issue.smart.branches.map((b) => `\`${b}\``).join(", ")}`,
    );
  }
  if (issue.smart.commits.length > 0) {
    lines.push(
      `- **${isZh ? "关联提交" : "Commits"}**: ${issue.smart.commits.map((c) => `\`${c}\``).join(", ")}`,
    );
  }
  if (issue.smart.githubUrl) {
    lines.push(`- **GitHub**: ${issue.smart.githubUrl}`);
  }

  if (issue.openBlockedBy.length > 0) {
    lines.push("");
    lines.push(isZh ? "## 当前未解除的前置阻塞" : "## Active Blockers");
    for (const bid of issue.openBlockedBy) {
      const b = byId[bid];
      lines.push(`- \`${bid}\`${b ? ` — ${b.title} (${b.status})` : ""}`);
    }
  }

  if (issue.description.trim()) {
    lines.push("");
    lines.push(isZh ? "## 为什么做（背景与目标）" : "## Why (Description)");
    lines.push(issue.description.trim());
  }

  if (issue.acceptanceCriteria.trim()) {
    lines.push("");
    lines.push(isZh ? "## 验收标准" : "## Acceptance Criteria");
    lines.push(issue.acceptanceCriteria.trim());
  }

  if (issue.notes.trim()) {
    lines.push("");
    lines.push(isZh ? "## 实施与交接备注" : "## Notes");
    lines.push(issue.notes.trim());
  }

  lines.push("");
  if (snapshot.storageBackend === "dolt-cli") {
    lines.push(isZh ? "## 完成后的 Beads 收口命令" : "## Beads Bookkeeping");
    lines.push("```bash");
    lines.push(`bd update ${issue.id} --status in_progress --force`);
    lines.push(`bd close ${issue.id} --force --reason "<summary>" --suggest-next`);
    lines.push("```");
  } else {
    lines.push(
      isZh
        ? "## Beadline 协作协议（优先调用 MCP 工具，或用 bl CLI）"
        : "## Beadline Coordination Protocol (MCP tools preferred, or `bl` CLI)",
    );
    lines.push(
      isZh
        ? '1. **开工认领**：调用 `bl_claim({ issueId: "' +
            issue.id +
            '", summary: "<当前步骤>", scope: ["<目录/**>"] })`（或执行 `bl claim ' +
            issue.id +
            "`），留意返回的并发重叠预警。"
        : `1. **Claim on start**: call \`bl_claim({ issueId: "${issue.id}", summary: "<current step>", scope: ["<dir/**>"] })\` (or \`bl claim ${issue.id}\`) and inspect any overlap warnings.`,
    );
    lines.push(
      isZh
        ? '2. **需要人工确认或切换阶段**：调用 `bl_pulse({ state: "waiting_human", waitingReason: "<明确问题>" })` 或 `state: "working"`。'
        : '2. **Need human input / step change**: call `bl_pulse({ state: "waiting_human", waitingReason: "<specific question>" })` or `state: "working"`.',
    );
    lines.push(
      isZh
        ? `3. **完工交付**：调用 \`bl_finish({ issueId: "${issue.id}", proof: { summary: "<验证结论与测试结果>", refs: ["<commit/PR>"] } })\`（或 \`bl finish ${issue.id} --summary "<结论>"\`）。`
        : `3. **Finish with proof**: call \`bl_finish({ issueId: "${issue.id}", proof: { summary: "<verification summary>", refs: ["<commit/PR>"] } })\` (or \`bl finish ${issue.id} --summary "<summary>"\`).`,
    );
  }

  return lines.join("\n");
}
