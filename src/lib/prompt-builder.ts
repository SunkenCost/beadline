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
      ? `# 任务执行上下文：${issue.shortId} (${issue.id})`
      : `# Task Brief: ${issue.shortId} (${issue.id})`,
  );
  lines.push("");
  lines.push(
    `- **${isZh ? "项目" : "Workspace"}**: ${snapshot.repoName} (\`${snapshot.repoPath}\`)`,
  );
  lines.push(`- **${isZh ? "标题" : "Title"}**: ${issue.title}`);
  lines.push(
    `- **${isZh ? "类型 / 优先级 / 状态" : "Type / Priority / Status"}**: ${issue.issueType} · P${issue.priority} · ${issue.status}`,
  );

  if (issue.parentTitle && issue.parentShortId) {
    lines.push(
      `- **${isZh ? "所属主目标" : "Parent Epic"}**: ${issue.parentShortId} — ${issue.parentTitle}`,
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
    lines.push(isZh ? "## 任务说明" : "## Description");
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
  lines.push(isZh ? "## 完成后的 Beads 收口命令" : "## Beads Bookkeeping");
  lines.push("```bash");
  lines.push(`bd update ${issue.id} --status in_progress --force`);
  lines.push(`bd close ${issue.id} --force --reason "<summary>" --suggest-next`);
  lines.push("```");

  return lines.join("\n");
}
