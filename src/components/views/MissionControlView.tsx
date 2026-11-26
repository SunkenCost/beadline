import {
  AlertTriangle,
  Bot,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock,
  ExternalLink,
  FolderOpen,
  GitBranch,
  HelpCircle,
  Layers,
  Plug,
  RefreshCw,
  ShieldAlert,
  Sparkles,
  Unlink,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/Button";
import { StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { formatIpcError, ipcOpenExternalLink, ipcRevealPath } from "@/lib/ipc";
import { buildAgentPrompt } from "@/lib/prompt-builder";
import { useRadarStore } from "@/stores/useRadarStore";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { AttentionItem, ConflictAlert } from "@/types/ipc";

function formatAgeMinutes(iso: string): number {
  const ts = Date.parse(iso);
  if (Number.isNaN(ts)) return 0;
  return Math.max(0, Math.floor((Date.now() - ts) / 60_000));
}

export function MissionControlView() {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const activePath = useWorkspaceStore((s) => s.activePath);
  const snapshot = useWorkspaceStore((s) => s.snapshot);
  const byId = useWorkspaceStore((s) => s.byId);

  const radar = useRadarStore((s) => s.radar);
  const scanning = useRadarStore((s) => s.scanning);
  const refreshRadar = useRadarStore((s) => s.refreshRadar);
  const clearSessionWaiting = useRadarStore((s) => s.clearSessionWaiting);
  const releaseSession = useRadarStore((s) => s.releaseSession);

  const openDrawer = useUiStore((s) => s.openDrawer);
  const setSettingsOpen = useUiStore((s) => s.setSettingsOpen);
  const pushToast = useUiStore((s) => s.pushToast);

  const [showAllAttention, setShowAllAttention] = useState(false);
  const [expandedConflicts, setExpandedConflicts] = useState<Record<string, boolean>>({});
  const [busySession, setBusySession] = useState<string | null>(null);

  useEffect(() => {
    if (!activePath) return;
    if (!radar || radar.repoPath.toLowerCase() !== activePath.toLowerCase()) {
      void refreshRadar(activePath, true);
    }
  }, [activePath, radar, refreshRadar]);

  const unattributedDirtyWorktrees = useMemo(() => {
    if (!radar) return [];
    return radar.worktrees.filter(
      (wt) => wt.attribution === "unattributed" && (wt.modifiedCount > 0 || wt.untrackedCount > 0),
    );
  }, [radar]);

  const handleClearWaiting = async (sessionId: string) => {
    if (!activePath) return;
    setBusySession(sessionId);
    try {
      await clearSessionWaiting(activePath, sessionId);
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusySession(null);
    }
  };

  const handleRelease = async (sessionId: string) => {
    if (!activePath) return;
    setBusySession(sessionId);
    try {
      await releaseSession(activePath, sessionId);
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusySession(null);
    }
  };

  const handleCopyBlockerHandoff = async (issueId: string) => {
    if (!snapshot) return;
    const issue = byId[issueId];
    if (!issue) return;
    const prompt = buildAgentPrompt(issue, snapshot, byId, locale);
    await navigator.clipboard?.writeText(prompt);
    pushToast(t.drawer.copiedPrompt, "ok");
  };

  const attentionAll = radar?.attention ?? [];
  const attentionVisible = showAllAttention ? attentionAll : attentionAll.slice(0, 3);
  const sessions = radar?.sessions ?? [];
  const conflicts = radar?.conflicts ?? [];
  const blockers = radar?.topBlockers ?? [];
  const coverage = radar?.coverage;
  const coverageDegraded = Boolean(coverage && coverage.scanStatus !== "ok");

  const renderAttentionBadge = (item: AttentionItem) => {
    switch (item.kind) {
      case "waiting-human":
        return (
          <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--accent-border)] bg-[var(--accent-soft)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--accent)]">
            <HelpCircle className="h-3 w-3" />
            <span>{t.mission.kindWaitingHuman}</span>
          </span>
        );
      case "file-overlap":
        return (
          <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--danger)]/35 bg-[var(--danger)]/10 px-1.5 py-0.5 text-[11px] font-medium text-[var(--danger)]">
            <ShieldAlert className="h-3 w-3" />
            <span>{t.mission.kindFileOverlap}</span>
          </span>
        );
      case "progress-stale":
        return (
          <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--warning)]/35 bg-[var(--warning)]/10 px-1.5 py-0.5 text-[11px] font-medium text-[var(--warning)]">
            <Clock className="h-3 w-3" />
            <span>
              {t.mission.kindProgressStale} · {item.ageMinutes}m
            </span>
          </span>
        );
      case "heartbeat-stale":
        return (
          <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--fg-2)]">
            <Unlink className="h-3 w-3" />
            <span>
              {t.mission.kindHeartbeatStale} · {item.ageMinutes}m
            </span>
          </span>
        );
    }
  };

  const renderConflictTierBadge = (c: ConflictAlert) => {
    if (c.kind === "same-file-dirty") {
      return (
        <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--danger)]/35 bg-[var(--danger)]/10 px-1.5 py-0.5 text-[11px] font-medium text-[var(--danger)]">
          {t.mission.tierSameFile}
        </span>
      );
    }
    if (c.kind === "dirty-hits-declared-scope") {
      return (
        <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--warning)]/35 bg-[var(--warning)]/10 px-1.5 py-0.5 text-[11px] font-medium text-[var(--warning)]">
          {t.mission.tierDirtyScope}
        </span>
      );
    }
    return (
      <span className="inline-flex items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-1.5 py-0.5 text-[11px] font-medium text-[var(--fg-2)]">
        {t.mission.tierScopeOverlap}
      </span>
    );
  };

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <div className="mx-auto flex max-w-[1100px] flex-col gap-4">
        {/* Honest Coverage & Action Strip */}
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-3.5 py-2.5">
          <div className="flex min-w-0 flex-1 items-center gap-2 text-[12px]">
            {coverageDegraded ? (
              <AlertTriangle className="h-3.5 w-3.5 shrink-0 text-[var(--warning)]" />
            ) : (
              <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-[var(--success)]" />
            )}
            <span className="truncate text-[var(--fg-2)]">
              {coverage?.scanStatus === "failed"
                ? t.mission.coverageFailed
                : coverage?.scanStatus === "stale"
                  ? t.mission.coverageStale
                  : coverage?.scanStatus === "partial"
                    ? t.mission.coveragePartial
                    : `${t.mission.coveragePrefix} · ${coverage?.scannedWorktrees ?? 0}/${coverage?.totalWorktrees ?? 0} ${t.mission.coverageWorktrees}`}
            </span>
          </div>

          <div className="flex items-center gap-1.5">
            <Button size="sm" variant="outline" onClick={() => setSettingsOpen(true)}>
              <Plug className="h-3 w-3 text-[var(--accent)]" />
              <span>{t.mission.aiSetupBtn}</span>
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={!activePath || scanning}
              onClick={() => activePath && void refreshRadar(activePath, true)}
            >
              <RefreshCw className={`h-3 w-3 ${scanning ? "animate-spin" : ""}`} />
              <span>{scanning ? t.mission.scanning : t.mission.rescanBtn}</span>
            </Button>
          </div>
        </div>

        {/* Block 1: Needs Attention (Top 3 urgent human decisions) */}
        <section className="rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)]">
          <header className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-2.5">
            <div className="flex items-center gap-2">
              <h2 className="text-[13px] font-semibold text-[var(--fg)]">
                {t.mission.attentionTitle}
              </h2>
              <span className="font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                {attentionAll.length}
              </span>
            </div>
            {attentionAll.length > 3 && (
              <button
                type="button"
                onClick={() => setShowAllAttention((v) => !v)}
                className="text-[11.5px] text-[var(--fg-2)] hover:text-[var(--fg)] cursor-pointer"
              >
                {showAllAttention
                  ? t.mission.collapse
                  : `${t.mission.showAll} (${attentionAll.length})`}
              </button>
            )}
          </header>

          {attentionVisible.length === 0 ? (
            <div className="px-4 py-4 text-[12.5px] text-[var(--fg-3)]">
              {t.mission.attentionEmpty}
            </div>
          ) : (
            <div className="divide-y divide-[var(--border-subtle)]">
              {attentionVisible.map((item) => {
                const issue = item.issueId ? byId[item.issueId] : undefined;
                return (
                  <div
                    key={item.id}
                    className="flex flex-wrap items-center gap-2.5 px-4 py-2.5 text-[12.5px]"
                  >
                    {renderAttentionBadge(item)}
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5">
                        {issue && (
                          <button
                            type="button"
                            onClick={() => openDrawer(issue.id)}
                            className="shrink-0 font-mono-tabular text-[11.5px] text-[var(--fg-3)] hover:text-[var(--accent)] cursor-pointer"
                          >
                            {issue.shortId}
                          </button>
                        )}
                        <span className="truncate font-medium text-[var(--fg)]">
                          {item.reason || issue?.title || item.id}
                        </span>
                      </div>
                      {(item.harness || issue) && (
                        <div className="mt-0.5 truncate text-[11.5px] text-[var(--fg-3)]">
                          {item.harness ? `${item.harness}` : ""}
                          {item.harness && issue ? " · " : ""}
                          {issue ? issue.title : ""}
                        </div>
                      )}
                    </div>

                    <div className="flex shrink-0 items-center gap-1.5">
                      {item.threadUrl && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() =>
                            void ipcOpenExternalLink(item.threadUrl ?? "").catch((e) =>
                              pushToast(formatIpcError(e), "err"),
                            )
                          }
                        >
                          <ExternalLink className="h-3 w-3" />
                          <span>{t.mission.openThread}</span>
                        </Button>
                      )}
                      {item.issueId && (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => openDrawer(item.issueId ?? "")}
                        >
                          <span>{t.mission.openTask}</span>
                        </Button>
                      )}
                      {item.kind === "waiting-human" && item.sessionId && (
                        <Button
                          size="sm"
                          variant="primary"
                          disabled={busySession === item.sessionId}
                          onClick={() => void handleClearWaiting(item.sessionId ?? "")}
                        >
                          <span>{t.mission.markHandled}</span>
                        </Button>
                      )}
                      {item.kind === "heartbeat-stale" && item.sessionId && (
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={busySession === item.sessionId}
                          onClick={() => void handleRelease(item.sessionId ?? "")}
                        >
                          <span>{t.mission.releaseBtn}</span>
                        </Button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        {/* Middle 2-Column Grid: Who's Doing What + Change Overlaps */}
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          {/* Block 2: Who's Doing What (Active Sessions + Unattributed Edits) */}
          <section className="flex flex-col rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)]">
            <header className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-2.5">
              <div className="flex items-center gap-2">
                <Bot className="h-3.5 w-3.5 text-[var(--accent)]" />
                <h2 className="text-[13px] font-semibold text-[var(--fg)]">
                  {t.mission.sessionsTitle}
                </h2>
                <span className="font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                  {sessions.length}
                </span>
              </div>
            </header>

            <div className="divide-y divide-[var(--border-subtle)]">
              {sessions.length === 0 ? (
                <div className="px-4 py-4 text-[12px] leading-relaxed text-[var(--fg-3)]">
                  {t.mission.sessionsEmpty}
                </div>
              ) : (
                sessions.map((s) => {
                  const issue = s.issueId ? byId[s.issueId] : undefined;
                  const ageMin = formatAgeMinutes(s.lastSeenAt);
                  return (
                    <div key={s.sessionId} className="space-y-1.5 px-4 py-3 text-[12px]">
                      <div className="flex items-center gap-2">
                        <span
                          className={`h-2 w-2 shrink-0 rounded-full ${
                            s.staleHeartbeat
                              ? "bg-[var(--fg-3)]"
                              : s.state === "waiting_human"
                                ? "bg-[var(--warning)]"
                                : "bg-[var(--success)]"
                          }`}
                        />
                        <span className="rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-1.5 py-0.5 font-mono-tabular text-[10.5px] text-[var(--fg-2)]">
                          {s.harness}
                        </span>
                        {issue ? (
                          <button
                            type="button"
                            onClick={() => openDrawer(issue.id)}
                            className="min-w-0 flex-1 truncate text-left font-medium text-[var(--fg)] hover:text-[var(--accent)] cursor-pointer"
                          >
                            <span className="mr-1.5 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                              {issue.shortId}
                            </span>
                            {issue.title}
                          </button>
                        ) : (
                          <span className="min-w-0 flex-1 truncate font-mono-tabular text-[11.5px] text-[var(--fg-2)]">
                            {s.sessionId}
                          </span>
                        )}
                        <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                          {ageMin}
                          {t.mission.minutesAgo}
                        </span>
                      </div>

                      {s.summary && (
                        <div className="pl-4 text-[12px] text-[var(--fg-2)]">
                          {s.state === "waiting_human" && s.waitingReason
                            ? s.waitingReason
                            : s.summary}
                        </div>
                      )}

                      <div className="flex flex-wrap items-center gap-2 pl-4 text-[11px] text-[var(--fg-3)]">
                        <span className="inline-flex items-center gap-1 font-mono-tabular">
                          <GitBranch className="h-3 w-3" />
                          <span>{s.branch ?? t.mission.detachedHead}</span>
                        </span>
                        {s.staleHeartbeat && (
                          <span className="text-[var(--warning)]">{t.mission.staleTelemetry}</span>
                        )}
                        <div className="ml-auto flex items-center gap-1.5">
                          {s.worktreePath && (
                            <button
                              type="button"
                              onClick={() =>
                                void ipcRevealPath(s.worktreePath).catch((e) =>
                                  pushToast(formatIpcError(e), "err"),
                                )
                              }
                              className="inline-flex items-center gap-1 hover:text-[var(--fg)] cursor-pointer"
                            >
                              <FolderOpen className="h-3 w-3" />
                            </button>
                          )}
                          <button
                            type="button"
                            disabled={busySession === s.sessionId}
                            onClick={() => void handleRelease(s.sessionId)}
                            className="hover:text-[var(--danger)] cursor-pointer"
                          >
                            {t.mission.releaseBtn}
                          </button>
                        </div>
                      </div>
                    </div>
                  );
                })
              )}
            </div>

            {/* Unattributed Dirty Worktrees (Honest Passive Git Observation) */}
            {unattributedDirtyWorktrees.length > 0 && (
              <div className="border-t border-[var(--border)] bg-[var(--surface-subtle)] px-4 py-3">
                <div className="mb-1 flex items-center justify-between">
                  <span className="text-[11.5px] font-semibold text-[var(--fg-2)]">
                    {t.mission.unattributedTitle} ({unattributedDirtyWorktrees.length})
                  </span>
                </div>
                <p className="mb-2 text-[11px] text-[var(--fg-3)]">{t.mission.unattributedDesc}</p>
                <div className="space-y-2">
                  {unattributedDirtyWorktrees.map((wt) => {
                    const totalDirty = wt.modifiedCount + wt.untrackedCount;
                    return (
                      <div
                        key={wt.worktreeId}
                        className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2.5 py-2 text-[11.5px]"
                      >
                        <div className="flex items-center gap-2">
                          <GitBranch className="h-3 w-3 shrink-0 text-[var(--fg-3)]" />
                          <span className="truncate font-mono-tabular font-medium text-[var(--fg)]">
                            {wt.branch ?? `${t.mission.detachedHead} (${wt.head ?? "?"})`}
                          </span>
                          <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                            {totalDirty} {t.mission.dirtyCount}
                          </span>
                          <button
                            type="button"
                            onClick={() =>
                              void ipcRevealPath(wt.path).catch((e) =>
                                pushToast(formatIpcError(e), "err"),
                              )
                            }
                            title={wt.path}
                            className="ml-auto text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                          >
                            <FolderOpen className="h-3.5 w-3.5" />
                          </button>
                        </div>
                        {wt.dirtySample.length > 0 && (
                          <div className="mt-1 flex flex-wrap gap-1 font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                            {wt.dirtySample.slice(0, 3).map((p) => (
                              <span
                                key={p}
                                className="rounded-[var(--radius-xs)] bg-[var(--surface-subtle)] px-1.5 py-0.5"
                              >
                                {p}
                              </span>
                            ))}
                            {totalDirty > 3 && (
                              <span className="px-1 py-0.5">+{totalDirty - 3}</span>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </section>

          {/* Block 3: Change Overlaps (Evidence-Graded 3-Tier Radar) */}
          <section className="flex flex-col rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)]">
            <header className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-2.5">
              <div className="flex items-center gap-2">
                <ShieldAlert className="h-3.5 w-3.5 text-[var(--warning)]" />
                <h2 className="text-[13px] font-semibold text-[var(--fg)]">
                  {t.mission.conflictsTitle}
                </h2>
                <span className="font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                  {conflicts.length}
                </span>
              </div>
            </header>

            {conflicts.length === 0 ? (
              <div className="px-4 py-4 text-[12px] leading-relaxed text-[var(--fg-3)]">
                {coverageDegraded ? t.mission.conflictsUnverified : t.mission.conflictsEmpty}
              </div>
            ) : (
              <div className="divide-y divide-[var(--border-subtle)]">
                {conflicts.map((c) => {
                  const expanded = Boolean(expandedConflicts[c.id]);
                  const evidenceList =
                    c.evidence.paths.length > 0 ? c.evidence.paths : c.evidence.matchedScopes;
                  const firstLine = evidenceList[0] ?? "";
                  const hasMore = evidenceList.length > 1;

                  return (
                    <div key={c.id} className="space-y-2 px-4 py-3 text-[12px]">
                      <div className="flex flex-wrap items-center gap-1.5">
                        {renderConflictTierBadge(c)}
                        {c.evidence.lockfileOnly && (
                          <span className="rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-1.5 py-0.5 text-[10.5px] text-[var(--fg-3)]">
                            {t.mission.lockfileOnly}
                          </span>
                        )}
                      </div>

                      <div className="flex flex-wrap items-center gap-1.5 text-[12px] text-[var(--fg)]">
                        {c.participants.map((p, idx) => (
                          <span key={p.id} className="inline-flex items-center gap-1.5">
                            {idx > 0 && <span className="text-[var(--fg-3)]">×</span>}
                            {p.issueId ? (
                              <button
                                type="button"
                                onClick={() => openDrawer(p.issueId ?? "")}
                                className="rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-1.5 py-0.5 font-mono-tabular text-[11px] hover:border-[var(--border-strong)] hover:text-[var(--accent)] cursor-pointer"
                              >
                                {p.label}
                                {p.harness ? ` (${p.harness})` : ""}
                              </button>
                            ) : (
                              <span className="rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-1.5 py-0.5 font-mono-tabular text-[11px] text-[var(--fg-2)]">
                                {p.label}
                              </span>
                            )}
                          </span>
                        ))}
                      </div>

                      {firstLine && (
                        <div className="space-y-1">
                          <div className="flex items-center gap-1.5 font-mono-tabular text-[11px] text-[var(--fg-2)]">
                            <span className="truncate">{firstLine}</span>
                            {hasMore && (
                              <button
                                type="button"
                                onClick={() =>
                                  setExpandedConflicts((prev) => ({
                                    ...prev,
                                    [c.id]: !prev[c.id],
                                  }))
                                }
                                className="inline-flex shrink-0 items-center gap-0.5 font-sans text-[11px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                              >
                                {expanded ? (
                                  <ChevronDown className="h-3 w-3" />
                                ) : (
                                  <ChevronRight className="h-3 w-3" />
                                )}
                                <span>
                                  {expanded
                                    ? t.mission.collapsePaths
                                    : `+${evidenceList.length - 1}`}
                                </span>
                              </button>
                            )}
                          </div>
                          {expanded && (
                            <div className="space-y-0.5 rounded-[var(--radius-sm)] bg-[var(--surface-subtle)] p-2 font-mono-tabular text-[11px] text-[var(--fg-2)]">
                              {evidenceList.slice(1).map((p) => (
                                <div key={p} className="truncate">
                                  {p}
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </section>
        </div>

        {/* Block 4: Top Actionable Root Blocker */}
        <section className="rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)]">
          <header className="flex items-center justify-between border-b border-[var(--border-subtle)] px-4 py-2.5">
            <div className="flex items-center gap-2">
              <Layers className="h-3.5 w-3.5 text-[var(--accent)]" />
              <h2 className="text-[13px] font-semibold text-[var(--fg)]">
                {t.mission.blockerTitle}
              </h2>
            </div>
          </header>

          {blockers.length === 0 ? (
            <div className="px-4 py-4 text-[12px] text-[var(--fg-3)]">{t.mission.blockerEmpty}</div>
          ) : (
            <div className="divide-y divide-[var(--border-subtle)]">
              {blockers.slice(0, 3).map((b, idx) => (
                <div
                  key={b.issueId}
                  className={`flex flex-wrap items-center gap-3 px-4 py-3 ${
                    idx === 0 && b.actionable ? "bg-[var(--accent-soft)]/35" : ""
                  }`}
                >
                  <StatusRing lane={b.lane} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <button
                        type="button"
                        onClick={() => openDrawer(b.issueId)}
                        className="font-mono-tabular text-[11.5px] text-[var(--fg-3)] hover:text-[var(--accent)] cursor-pointer"
                      >
                        {b.shortId}
                      </button>
                      <button
                        type="button"
                        onClick={() => openDrawer(b.issueId)}
                        className="truncate text-left text-[13px] font-medium text-[var(--fg)] hover:text-[var(--accent)] cursor-pointer"
                      >
                        {b.title}
                      </button>
                      <span
                        className={`rounded-[var(--radius-xs)] border px-1.5 py-0.5 text-[10.5px] ${
                          b.actionable
                            ? "border-[var(--success)]/35 bg-[var(--success-soft)] text-[var(--success)]"
                            : "border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--fg-3)]"
                        }`}
                      >
                        {b.actionable ? t.mission.actionableNow : t.mission.blockedUpstream}
                      </span>
                    </div>
                    <div className="mt-1 flex flex-wrap items-center gap-3 text-[11.5px] text-[var(--fg-2)]">
                      <span>
                        {t.mission.directUnblocks}{" "}
                        <strong className="font-mono-tabular text-[var(--fg)]">
                          {b.directUnblocks}
                        </strong>
                      </span>
                      <span>
                        {t.mission.totalDownstream}{" "}
                        <strong className="font-mono-tabular text-[var(--fg)]">
                          {b.totalDownstream}
                        </strong>
                      </span>
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-1.5">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void handleCopyBlockerHandoff(b.issueId)}
                    >
                      <Sparkles className="h-3 w-3 text-[var(--accent)]" />
                      <span>{t.mission.handoffBtn}</span>
                    </Button>
                    <Button size="sm" variant="primary" onClick={() => openDrawer(b.issueId)}>
                      <span>{t.actions.details}</span>
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
