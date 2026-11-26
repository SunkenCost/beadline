import type {
  BoardLane,
  BoardStats,
  IssueDto,
  ModuleCount,
  UpdateIssueRequest,
  WorkspaceSnapshot,
} from "@/types/ipc";

function deriveLane(status: string, hasOpenBlockers: boolean): BoardLane {
  if (status === "closed") return "closed";
  if (status === "in_progress") return "in_progress";
  if (status === "blocked") return "blocked";
  return hasOpenBlockers ? "blocked" : "ready";
}

function recomputeSnapshot(prev: WorkspaceSnapshot, nextIssues: IssueDto[]): WorkspaceSnapshot {
  const statusMap = new Map<string, string>();
  for (const it of nextIssues) {
    statusMap.set(it.id, it.status);
  }

  const stats: BoardStats = {
    total: 0,
    inProgress: 0,
    ready: 0,
    blocked: 0,
    closed: 0,
    epics: 0,
  };
  const modCounts = new Map<string, number>();
  const prefix = prev.modulePrefix || "mod:";

  const normalized: IssueDto[] = nextIssues.map((it) => {
    const openBlockedBy = it.blockedBy.filter((bid) => statusMap.get(bid) !== "closed");
    const lane = deriveLane(it.status, openBlockedBy.length > 0);
    const modules = [...new Set(it.labels.filter((l) => l.startsWith(prefix)))].sort();

    stats.total += 1;
    if (lane === "in_progress") stats.inProgress += 1;
    else if (lane === "ready") stats.ready += 1;
    else if (lane === "blocked") stats.blocked += 1;
    else stats.closed += 1;
    if (it.issueType === "epic") stats.epics += 1;

    for (const m of modules) {
      modCounts.set(m, (modCounts.get(m) ?? 0) + 1);
    }

    return {
      ...it,
      lane,
      openBlockedBy,
      modules,
    };
  });

  const modules: ModuleCount[] = [...modCounts.entries()]
    .map(([key, count]) => ({ key, count }))
    .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key));

  const epics = normalized.filter((i) => i.issueType === "epic");

  return {
    ...prev,
    stats,
    modules,
    issues: normalized,
    epics,
  };
}

export function applyOptimisticUpdate(
  snapshot: WorkspaceSnapshot,
  patch: Omit<UpdateIssueRequest, "workspacePath">,
): WorkspaceSnapshot {
  const nextIssues = snapshot.issues.map((it) => {
    if (it.id !== patch.issueId) return it;
    return {
      ...it,
      title: patch.title ?? it.title,
      status: patch.status ?? it.status,
      priority: patch.priority ?? it.priority,
      assignee: patch.assignee ?? it.assignee,
      labels: patch.labels ?? it.labels,
      description: patch.description ?? it.description,
      acceptanceCriteria: patch.acceptanceCriteria ?? it.acceptanceCriteria,
      notes: patch.notes ?? it.notes,
      scope: patch.scope ?? it.scope,
      updatedAt: new Date().toISOString(),
    };
  });

  return recomputeSnapshot(snapshot, nextIssues);
}

export function applyOptimisticClose(
  snapshot: WorkspaceSnapshot,
  issueId: string,
  reason?: string,
): WorkspaceSnapshot {
  const now = new Date().toISOString();
  const nextIssues = snapshot.issues.map((it) => {
    if (it.id !== issueId) return it;
    return {
      ...it,
      status: "closed",
      closedAt: now,
      updatedAt: now,
      closeReason: reason?.trim() || it.closeReason,
    };
  });
  return recomputeSnapshot(snapshot, nextIssues);
}

export function applyOptimisticReopen(
  snapshot: WorkspaceSnapshot,
  issueId: string,
): WorkspaceSnapshot {
  const now = new Date().toISOString();
  const nextIssues = snapshot.issues.map((it) => {
    if (it.id !== issueId) return it;
    return {
      ...it,
      status: "open",
      closedAt: "",
      updatedAt: now,
    };
  });
  return recomputeSnapshot(snapshot, nextIssues);
}
