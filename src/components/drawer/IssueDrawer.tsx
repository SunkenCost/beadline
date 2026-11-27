import {
  Bot,
  Check,
  CheckCircle2,
  Copy,
  ExternalLink,
  FolderOpen,
  GitBranch,
  GitCommitHorizontal,
  Layers,
  MessageSquare,
  Sparkles,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/Button";
import { StatusRing } from "@/components/ui/Icons";
import { MiniMarkdown } from "@/components/ui/MiniMarkdown";
import { useT } from "@/i18n";
import { extractPlainSummary } from "@/lib/human-context";
import { formatIpcError, ipcGetIssueComments, ipcOpenExternalLink, ipcRevealPath } from "@/lib/ipc";
import { resolveModule } from "@/lib/modules";
import { buildAgentPrompt } from "@/lib/prompt-builder";
import { useRadarStore } from "@/stores/useRadarStore";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { CommentDto, IssueDto, ModuleAliasConfig } from "@/types/ipc";

const DEFAULT_MOD_SUGGESTIONS = [
  "engine",
  "desktop",
  "ui",
  "agent",
  "kg",
  "mcp",
  "release",
  "docs",
];

export interface IssueDrawerProps {
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
  onRequestClose: (issue: IssueDto) => void;
}

export function IssueDrawer({ modulePrefix, moduleAliases, onRequestClose }: IssueDrawerProps) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const selectedIssueId = useUiStore((s) => s.selectedIssueId);
  const openDrawer = useUiStore((s) => s.openDrawer);
  const closeDrawer = useUiStore((s) => s.closeDrawer);
  const pushToast = useUiStore((s) => s.pushToast);

  const activePath = useWorkspaceStore((s) => s.activePath);
  const snapshot = useWorkspaceStore((s) => s.snapshot);
  const byId = useWorkspaceStore((s) => s.byId);
  const updateIssue = useWorkspaceStore((s) => s.updateIssue);
  const reopenIssue = useWorkspaceStore((s) => s.reopenIssue);
  const addDependency = useWorkspaceStore((s) => s.addDependency);
  const removeDependency = useWorkspaceStore((s) => s.removeDependency);
  const addComment = useWorkspaceStore((s) => s.addComment);

  const sessionsByIssueId = useRadarStore((s) => s.sessionsByIssueId);
  const clearSessionWaiting = useRadarStore((s) => s.clearSessionWaiting);
  const releaseSession = useRadarStore((s) => s.releaseSession);

  const issue = selectedIssueId ? (byId[selectedIssueId] ?? null) : null;
  const issueId = issue?.id ?? null;
  const boundSessions = issueId ? (sessionsByIssueId[issueId] ?? []) : [];
  const issueUpdatedAt = issue?.updatedAt ?? "";
  const issueCommentCount = issue?.commentCount ?? 0;

  const [titleDraft, setTitleDraft] = useState("");
  const [assigneeDraft, setAssigneeDraft] = useState("");
  const [labelsDraft, setLabelsDraft] = useState("");
  const [scopeDraft, setScopeDraft] = useState("");
  const [editingTexts, setEditingTexts] = useState(false);
  const [descDraft, setDescDraft] = useState("");
  const [accDraft, setAccDraft] = useState("");
  const [notesDraft, setNotesDraft] = useState("");
  const [newModOpen, setNewModOpen] = useState(false);
  const [newModDraft, setNewModDraft] = useState("");
  const [blockerPick, setBlockerPick] = useState("");
  const [comments, setComments] = useState<CommentDto[]>([]);
  const [commentDraft, setCommentDraft] = useState("");
  const [busy, setBusy] = useState(false);

  const prefix = modulePrefix || "mod:";

  const ownMods = useMemo(
    () => new Set((issue?.labels ?? []).filter((l) => l.startsWith(prefix))),
    [issue?.labels, prefix],
  );
  const nonModLabels = useMemo(
    () => (issue?.labels ?? []).filter((l) => !l.startsWith(prefix)),
    [issue?.labels, prefix],
  );

  useEffect(() => {
    if (!issueId) return;
    const current = useWorkspaceStore.getState().byId[issueId];
    if (!current || current.updatedAt !== issueUpdatedAt) return;
    setTitleDraft(current.title);
    setAssigneeDraft(current.assignee);
    setLabelsDraft(current.labels.filter((l) => !l.startsWith(prefix)).join(", "));
    setScopeDraft((current.scope ?? []).join(", "));
    setDescDraft(current.description);
    setAccDraft(current.acceptanceCriteria);
    setNotesDraft(current.notes);
    setEditingTexts(false);
    setNewModOpen(false);
    setNewModDraft("");
    setBlockerPick("");
  }, [issueId, issueUpdatedAt, prefix]);

  useEffect(() => {
    // Skip spawning `bd --readonly comments` when the issue has zero comments,
    // and depend strictly on primitive identity/count rather than object reference.
    if (!issueId || !activePath || issueCommentCount === 0) {
      setComments([]);
      return;
    }
    let alive = true;
    void ipcGetIssueComments(activePath, issueId)
      .then((list) => {
        if (alive) setComments(list);
      })
      .catch(() => {
        if (alive) setComments([]);
      });
    return () => {
      alive = false;
    };
  }, [activePath, issueId, issueCommentCount]);

  const modChoices = useMemo(() => {
    const set = new Set<string>();
    for (const m of snapshot?.modules ?? []) set.add(m.key);
    for (const m of ownMods) set.add(m);
    if (set.size < 4) {
      for (const s of DEFAULT_MOD_SUGGESTIONS.slice(0, 5)) set.add(`${prefix}${s}`);
    }
    return [...set];
  }, [snapshot?.modules, ownMods, prefix]);

  if (!issue || !snapshot) return null;

  const parentIssue = issue.parentId ? (byId[issue.parentId] ?? null) : null;
  const parentSummary = parentIssue ? extractPlainSummary(parentIssue.description, 160) : "";
  const promoteCloseReason = !issue.description.trim() && Boolean(issue.closeReason.trim());

  const blockers = issue.blockedBy.map((bid) => byId[bid]).filter((x): x is IssueDto => Boolean(x));
  const unlocks = issue.blocks.map((uid) => byId[uid]).filter((x): x is IssueDto => Boolean(x));
  const candidates = snapshot.issues.filter(
    (x) => x.id !== issue.id && !issue.blockedBy.includes(x.id),
  );

  const patch = async (fields: Parameters<typeof updateIssue>[0], toastMsg: string) => {
    setBusy(true);
    try {
      await updateIssue(fields);
      pushToast(toastMsg, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusy(false);
    }
  };

  const handleToggleMod = async (modKey: string) => {
    const next = new Set(ownMods);
    if (next.has(modKey)) next.delete(modKey);
    else next.add(modKey);
    await patch({ issueId: issue.id, labels: [...next, ...nonModLabels] }, t.settings.savedHint);
  };

  const handleAddCustomMod = async () => {
    const slug = newModDraft.trim().replace(new RegExp(`^${prefix}`), "");
    if (!slug) return;
    const next = new Set(ownMods);
    next.add(`${prefix}${slug}`);
    setNewModDraft("");
    setNewModOpen(false);
    await patch({ issueId: issue.id, labels: [...next, ...nonModLabels] }, t.settings.savedHint);
  };

  const handleCopyPrompt = async () => {
    const prompt = buildAgentPrompt(issue, snapshot, byId, locale);
    await navigator.clipboard?.writeText(prompt);
    pushToast(t.drawer.copiedPrompt, "ok");
  };

  const handleAddBlocker = async () => {
    if (!blockerPick) return;
    setBusy(true);
    try {
      await addDependency(issue.id, blockerPick);
      setBlockerPick("");
      pushToast(t.settings.savedHint, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusy(false);
    }
  };

  const handleRemoveBlocker = async (blockerId: string) => {
    setBusy(true);
    try {
      await removeDependency(issue.id, blockerId);
      pushToast(t.settings.savedHint, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusy(false);
    }
  };

  const handleSendComment = async () => {
    const text = commentDraft.trim();
    if (!text) return;
    setBusy(true);
    try {
      await addComment(issue.id, text);
      setCommentDraft("");
      if (activePath) {
        const refreshed = await ipcGetIssueComments(activePath, issue.id).catch(() => comments);
        setComments(refreshed);
      }
      pushToast(t.settings.savedHint, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusy(false);
    }
  };

  const handleReopen = async () => {
    setBusy(true);
    try {
      await reopenIssue(issue.id, t.drawer.reopenReasonDefault);
      pushToast(t.actions.reopen, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div
        aria-hidden="true"
        onClick={closeDrawer}
        className="fixed inset-0 z-40 bg-black/30 transition-opacity"
      />
      <aside className="fixed top-10 right-0 bottom-0 z-45 flex w-[min(560px,100vw)] flex-col border-l border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-lg)]">
        {/* Drawer Header */}
        <header className="flex h-11 shrink-0 items-center gap-2 border-b border-[var(--border)] px-4">
          <button
            type="button"
            onClick={() => {
              void navigator.clipboard?.writeText(issue.id);
              pushToast(`${t.drawer.copiedId}: ${issue.id}`, "ok");
            }}
            className="shrink-0 whitespace-nowrap font-mono-tabular text-[11.5px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
          >
            {issue.shortId}
          </button>
          <span className="text-[var(--fg-3)]">·</span>
          <span className="shrink-0 whitespace-nowrap text-[12px] text-[var(--fg-2)]">
            {t.types[issue.issueType as keyof typeof t.types] ?? issue.issueType}
          </span>

          <div className="ml-auto flex items-center gap-1.5">
            <Button size="sm" variant="accent" onClick={() => void handleCopyPrompt()}>
              <Sparkles className="h-3 w-3" />
              <span>{t.actions.copyAgentPrompt}</span>
            </Button>
            <Button
              size="icon"
              variant="ghost"
              aria-label={t.actions.close}
              onClick={closeDrawer}
              className="h-6 w-6"
            >
              <X className="h-3.5 w-3.5" />
            </Button>
          </div>
        </header>

        {/* Drawer Body — Content & 1-click actions FIRST, form fields below */}
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-5 py-4">
          <input
            type="text"
            value={titleDraft}
            onChange={(e) => setTitleDraft(e.target.value)}
            onBlur={() => {
              const next = titleDraft.trim();
              if (next && next !== issue.title) {
                void patch({ issueId: issue.id, title: next }, t.settings.savedHint);
              }
            }}
            className="-mx-1.5 w-[calc(100%+12px)] rounded-[var(--radius-sm)] border border-transparent px-1.5 py-1 text-[16px] font-semibold text-[var(--fg)] focus:border-[var(--border-strong)] focus:bg-[var(--surface-subtle)]"
          />

          {/* 1-Click Status & Priority Quick Switch Bar */}
          <div className="flex flex-wrap items-center gap-2">
            <div className="inline-flex items-center rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
              {(["open", "in_progress", "blocked", "closed"] as const).map((st) => {
                const active = issue.status === st;
                return (
                  <button
                    key={st}
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      if (st === issue.status) return;
                      if (st === "closed") {
                        onRequestClose(issue);
                      } else {
                        void patch({ issueId: issue.id, status: st }, t.settings.savedHint);
                      }
                    }}
                    className={`inline-flex h-6 items-center gap-1 whitespace-nowrap rounded-[var(--radius-xs)] px-2 text-[11.5px] transition-colors cursor-pointer ${
                      active
                        ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                        : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                    }`}
                  >
                    <StatusRing lane={st === "open" ? "ready" : st} size={11} />
                    <span>{t.statuses[st]}</span>
                  </button>
                );
              })}
            </div>

            <div className="inline-flex items-center rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
              {[0, 1, 2, 3, 4].map((p) => {
                const active = issue.priority === p;
                return (
                  <button
                    key={p}
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      if (p !== issue.priority) {
                        void patch({ issueId: issue.id, priority: p }, t.settings.savedHint);
                      }
                    }}
                    className={`h-6 whitespace-nowrap rounded-[var(--radius-xs)] px-1.5 font-mono-tabular text-[11px] transition-colors cursor-pointer ${
                      active
                        ? "bg-[var(--surface)] font-semibold text-[var(--fg)] shadow-[var(--shadow-xs)]"
                        : "text-[var(--fg-3)] hover:text-[var(--fg)]"
                    }`}
                  >
                    P{p}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Active Agent Sessions bound to this issue */}
          {boundSessions.length > 0 && (
            <div className="space-y-2 rounded-[var(--radius-md)] border border-[var(--accent-border)] bg-[var(--accent-soft)]/35 px-3.5 py-2.5">
              <div className="flex items-center gap-1.5 text-[11px] font-semibold text-[var(--fg-2)]">
                <Bot className="h-3.5 w-3.5 text-[var(--accent)]" />
                <span>
                  {t.drawer.activeSessions} ({boundSessions.length})
                </span>
              </div>
              <div className="space-y-1.5">
                {boundSessions.map((s) => (
                  <div
                    key={s.sessionId}
                    className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2.5 py-2 text-[11.5px]"
                  >
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span
                        className={`h-1.5 w-1.5 rounded-full ${
                          s.staleHeartbeat
                            ? "bg-[var(--fg-3)]"
                            : s.state === "waiting_human"
                              ? "bg-[var(--warning)]"
                              : "bg-[var(--success)]"
                        }`}
                      />
                      <span className="font-mono-tabular font-medium text-[var(--fg)]">
                        {s.harness}
                      </span>
                      {s.branch && (
                        <span className="font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                          {s.branch}
                        </span>
                      )}
                      {s.state === "waiting_human" && (
                        <span className="rounded-[var(--radius-xs)] bg-[var(--accent-soft)] px-1.5 py-0.5 text-[10.5px] font-medium text-[var(--accent)]">
                          {t.drawer.waitingHumanBadge}
                        </span>
                      )}
                      <div className="ml-auto flex items-center gap-1">
                        {s.state === "waiting_human" && activePath && (
                          <Button
                            size="sm"
                            variant="primary"
                            onClick={() => void clearSessionWaiting(activePath, s.sessionId)}
                          >
                            {t.drawer.clearWaiting}
                          </Button>
                        )}
                        {activePath && (
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => void releaseSession(activePath, s.sessionId)}
                          >
                            {t.drawer.releaseSession}
                          </Button>
                        )}
                      </div>
                    </div>
                    {(s.waitingReason || s.summary) && (
                      <div className="mt-1 text-[11.5px] text-[var(--fg-2)]">
                        {s.state === "waiting_human" && s.waitingReason
                          ? s.waitingReason
                          : s.summary}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Human-readable Context & Purpose Card — immediately answers "what is this task for?" */}
          {(parentIssue || promoteCloseReason) && (
            <div className="space-y-2 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] px-3.5 py-2.5">
              <div className="text-[11px] font-medium text-[var(--fg-3)]">
                {t.drawer.contextHeading}
              </div>
              {parentIssue && (
                <div className="space-y-1">
                  <button
                    type="button"
                    onClick={() => openDrawer(parentIssue.id)}
                    className="flex w-full items-start gap-2 text-left text-[12.5px] font-medium text-[var(--fg)] hover:text-[var(--accent)] cursor-pointer"
                  >
                    <Layers className="mt-0.5 h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
                    <span className="min-w-0 flex-1">
                      <span className="mr-1.5 font-normal text-[var(--fg-3)]">
                        {t.drawer.contextParentLabel}:
                      </span>
                      {parentIssue.title}
                    </span>
                  </button>
                  {parentSummary && (
                    <p className="pl-5.5 text-[12px] leading-relaxed text-[var(--fg-2)]">
                      {parentSummary}
                    </p>
                  )}
                </div>
              )}
              {promoteCloseReason && (
                <div className="border-t border-[var(--border-subtle)] pt-2">
                  <div className="mb-1 text-[11px] font-medium text-[var(--success)]">
                    {t.drawer.closeReason}
                  </div>
                  <MiniMarkdown content={issue.closeReason} tone="success" />
                </div>
              )}
            </div>
          )}

          {/* Smart extracted links, branches, commits, worktrees */}
          {(issue.smart.githubUrl ||
            issue.smart.branches.length > 0 ||
            issue.smart.commits.length > 0 ||
            issue.smart.worktrees.length > 0 ||
            issue.smart.links.length > 0) && (
            <div className="flex flex-wrap gap-1.5">
              {issue.smart.githubUrl && issue.smart.githubNumber && (
                <button
                  type="button"
                  onClick={() => void ipcOpenExternalLink(issue.smart.githubUrl ?? "")}
                  className="inline-flex h-6 items-center gap-1.5 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2 font-mono-tabular text-[11px] text-[var(--fg)] hover:border-[var(--border-strong)] cursor-pointer"
                >
                  <span>GitHub #{issue.smart.githubNumber}</span>
                  <ExternalLink className="h-3 w-3" />
                </button>
              )}
              {issue.smart.branches.map((br) => (
                <button
                  key={br}
                  type="button"
                  onClick={() => {
                    void navigator.clipboard?.writeText(br);
                    pushToast(br, "ok");
                  }}
                  className="inline-flex h-6 items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2 font-mono-tabular text-[11px] text-[var(--fg-2)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] cursor-pointer"
                >
                  <GitBranch className="h-3 w-3" />
                  <span>{br}</span>
                </button>
              ))}
              {issue.smart.commits.map((sha) => (
                <button
                  key={sha}
                  type="button"
                  onClick={() => {
                    void navigator.clipboard?.writeText(sha);
                    pushToast(sha, "ok");
                  }}
                  className="inline-flex h-6 items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2 font-mono-tabular text-[11px] text-[var(--fg-2)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] cursor-pointer"
                >
                  <GitCommitHorizontal className="h-3 w-3" />
                  <span>{sha}</span>
                </button>
              ))}
              {issue.smart.worktrees.map((wt) => (
                <button
                  key={wt.path}
                  type="button"
                  onClick={() =>
                    void ipcRevealPath(wt.path).catch((e) => pushToast(formatIpcError(e), "err"))
                  }
                  title={wt.path}
                  className="inline-flex h-6 items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2 text-[11px] text-[var(--fg-2)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] cursor-pointer"
                >
                  <FolderOpen className="h-3 w-3" />
                  <span>{t.drawer.revealWorktree}</span>
                </button>
              ))}
              {issue.smart.links.map((lk, idx) => (
                <div key={lk.url} className="inline-flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() =>
                      void ipcOpenExternalLink(lk.url).catch((e) =>
                        pushToast(formatIpcError(e), "err"),
                      )
                    }
                    title={lk.url}
                    className="inline-flex h-6 items-center gap-1 rounded-[var(--radius-xs)] border border-[var(--accent-border)] bg-[var(--accent-soft)] px-2 text-[11px] font-medium text-[var(--accent)] hover:brightness-105 cursor-pointer"
                  >
                    <MessageSquare className="h-3 w-3" />
                    <span>
                      {lk.label}
                      {issue.smart.links.length > 1 ? ` #${idx + 1}` : ""}
                    </span>
                  </button>
                  <button
                    type="button"
                    aria-label={t.actions.copyId}
                    onClick={() => {
                      void navigator.clipboard?.writeText(lk.url);
                      pushToast(lk.url.slice(0, 48), "ok");
                    }}
                    className="grid h-6 w-6 place-items-center rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                  >
                    <Copy className="h-3 w-3" />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Description / Acceptance / Notes / Proof — placed right up top for instant reading */}
          <section className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                {t.drawer.description}
              </span>
              <Button size="sm" variant="ghost" onClick={() => setEditingTexts((v) => !v)}>
                {editingTexts ? t.actions.cancel : t.actions.edit}
              </Button>
            </div>

            {editingTexts ? (
              <div className="space-y-2.5">
                <textarea
                  rows={5}
                  value={descDraft}
                  onChange={(e) => setDescDraft(e.target.value)}
                  className="w-full rounded-[var(--radius-sm)] border border-[var(--border-strong)] bg-[var(--surface)] p-2.5 text-[12.5px] leading-relaxed"
                />
                <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                  {t.drawer.acceptance}
                </div>
                <textarea
                  rows={2}
                  value={accDraft}
                  onChange={(e) => setAccDraft(e.target.value)}
                  className="w-full rounded-[var(--radius-sm)] border border-[var(--border-strong)] bg-[var(--surface)] p-2.5 text-[12.5px] leading-relaxed"
                />
                <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                  {t.drawer.notes}
                </div>
                <textarea
                  rows={3}
                  value={notesDraft}
                  onChange={(e) => setNotesDraft(e.target.value)}
                  className="w-full rounded-[var(--radius-sm)] border border-[var(--border-strong)] bg-[var(--surface)] p-2.5 text-[12.5px] leading-relaxed"
                />
                <div className="flex justify-end">
                  <Button
                    variant="primary"
                    disabled={busy}
                    onClick={() => {
                      setEditingTexts(false);
                      void patch(
                        {
                          issueId: issue.id,
                          description: descDraft,
                          acceptanceCriteria: accDraft,
                          notes: notesDraft,
                        },
                        t.settings.savedHint,
                      );
                    }}
                  >
                    {t.actions.save}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="space-y-2.5">
                <MiniMarkdown
                  content={issue.description}
                  emptyText={parentIssue ? t.drawer.contextEmptyDescHint : t.drawer.noContent}
                />

                {issue.acceptanceCriteria.trim() && (
                  <>
                    <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                      {t.drawer.acceptance}
                    </div>
                    <MiniMarkdown content={issue.acceptanceCriteria} />
                  </>
                )}

                {issue.notes.trim() && (
                  <>
                    <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                      {t.drawer.notes}
                    </div>
                    <MiniMarkdown content={issue.notes} tone="muted" />
                  </>
                )}

                {issue.proof && (
                  <div className="rounded-[var(--radius-sm)] border border-[var(--success)]/30 bg-[var(--success-soft)]/40 p-2.5">
                    <div className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold text-[var(--success)]">
                      <CheckCircle2 className="h-3.5 w-3.5" />
                      <span>{t.drawer.proof}</span>
                    </div>
                    <MiniMarkdown content={issue.proof.summary} tone="success" />
                    {issue.proof.refs.length > 0 && (
                      <div className="mt-1.5 flex flex-wrap gap-1 font-mono-tabular text-[10.5px] text-[var(--fg-2)]">
                        {issue.proof.refs.map((r) => (
                          <span
                            key={r}
                            className="rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-1.5 py-0.5"
                          >
                            {r}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                )}

                {issue.closeReason.trim() && !promoteCloseReason && !issue.proof && (
                  <>
                    <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                      {t.drawer.closeReason}
                    </div>
                    <MiniMarkdown content={issue.closeReason} tone="success" />
                  </>
                )}
              </div>
            )}
          </section>

          {/* Parent Epic fallback if parent record isn't loaded in byId */}
          {!parentIssue && issue.parentId && issue.parentTitle && (
            <button
              type="button"
              onClick={() => openDrawer(issue.parentId ?? "")}
              className="flex w-full items-center gap-2 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 py-2 text-left text-[12px] hover:border-[var(--border-strong)] cursor-pointer"
            >
              <Layers className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
              <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                {issue.parentShortId}
              </span>
              <span className="min-w-0 flex-1 truncate font-medium text-[var(--fg)]">
                {issue.parentTitle}
              </span>
            </button>
          )}

          {/* Blockers */}
          <section className="space-y-1.5">
            <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
              {t.drawer.blockers} ({blockers.length})
            </div>
            {blockers.length === 0 ? (
              <div className="text-[12px] text-[var(--fg-3)]">{t.drawer.noBlockers}</div>
            ) : (
              <div className="space-y-1">
                {blockers.map((b) => (
                  <div
                    key={b.id}
                    className="flex items-center gap-2 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 py-1.5 text-[12px]"
                  >
                    <button
                      type="button"
                      onClick={() => openDrawer(b.id)}
                      className="flex min-w-0 flex-1 items-center gap-2 text-left hover:text-[var(--accent)] cursor-pointer"
                    >
                      <StatusRing lane={b.lane} />
                      <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                        {b.shortId}
                      </span>
                      <span className="truncate font-medium text-[var(--fg)]">{b.title}</span>
                    </button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      onClick={() => void handleRemoveBlocker(b.id)}
                    >
                      {t.actions.remove}
                    </Button>
                  </div>
                ))}
              </div>
            )}
            <div className="flex items-center gap-1.5 pt-0.5">
              <select
                value={blockerPick}
                onChange={(e) => setBlockerPick(e.target.value)}
                className="h-7 min-w-0 flex-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 text-[12px] text-[var(--fg)]"
              >
                <option value="">{t.drawer.selectBlocker}</option>
                {candidates.map((c) => (
                  <option key={c.id} value={c.id}>
                    [{t.lanes[c.lane]}] {c.shortId} · {c.title.slice(0, 36)}
                  </option>
                ))}
              </select>
              <Button
                size="sm"
                variant="outline"
                disabled={!blockerPick || busy}
                onClick={() => void handleAddBlocker()}
                className="h-7"
              >
                + {t.actions.add}
              </Button>
            </div>
          </section>

          {/* Downstream unlocks */}
          {unlocks.length > 0 && (
            <section className="space-y-1.5">
              <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
                {t.drawer.unlocks} ({unlocks.length})
              </div>
              <div className="space-y-1">
                {unlocks.map((u) => (
                  <button
                    key={u.id}
                    type="button"
                    onClick={() => openDrawer(u.id)}
                    className="flex w-full items-center gap-2 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 py-1.5 text-left text-[12px] hover:border-[var(--border-strong)] cursor-pointer"
                  >
                    <StatusRing lane={u.lane} />
                    <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                      {u.shortId}
                    </span>
                    <span className="min-w-0 flex-1 truncate font-medium text-[var(--fg)]">
                      {u.title}
                    </span>
                  </button>
                ))}
              </div>
            </section>
          )}

          {/* Compact Properties & Modules section */}
          <section className="space-y-2">
            <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
              {t.drawer.propertiesHeading}
            </div>
            <div className="grid grid-cols-2 gap-2.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] p-3">
              <div className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-[var(--fg-3)]">
                  {t.drawer.assignee}
                </span>
                <input
                  type="text"
                  value={assigneeDraft}
                  onChange={(e) => setAssigneeDraft(e.target.value)}
                  onBlur={() => {
                    if (assigneeDraft.trim() !== issue.assignee) {
                      void patch(
                        { issueId: issue.id, assignee: assigneeDraft.trim() },
                        t.settings.savedHint,
                      );
                    }
                  }}
                  placeholder={t.drawer.assigneePlaceholder}
                  className="h-7 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 font-mono-tabular text-[12px] text-[var(--fg)]"
                />
              </div>

              <div className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-[var(--fg-3)]">
                  {t.drawer.otherLabels}
                </span>
                <input
                  type="text"
                  value={labelsDraft}
                  onChange={(e) => setLabelsDraft(e.target.value)}
                  onBlur={() => {
                    const others = labelsDraft
                      .split(",")
                      .map((s) => s.trim())
                      .filter(Boolean);
                    const merged = [...new Set([...ownMods, ...others])];
                    if (merged.join(",") !== issue.labels.join(",")) {
                      void patch({ issueId: issue.id, labels: merged }, t.settings.savedHint);
                    }
                  }}
                  placeholder={t.drawer.labelsPlaceholder}
                  className="h-7 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 font-mono-tabular text-[12px] text-[var(--fg)]"
                />
              </div>

              {snapshot.storageBackend === "native-jsonl" && (
                <div className="col-span-2 flex flex-col gap-1">
                  <span className="text-[11px] font-medium text-[var(--fg-3)]">
                    {t.drawer.scope}
                  </span>
                  <input
                    type="text"
                    value={scopeDraft}
                    onChange={(e) => setScopeDraft(e.target.value)}
                    onBlur={() => {
                      const nextScope = scopeDraft
                        .split(",")
                        .map((s) => s.trim())
                        .filter(Boolean);
                      if (nextScope.join(",") !== (issue.scope ?? []).join(",")) {
                        void patch({ issueId: issue.id, scope: nextScope }, t.settings.savedHint);
                      }
                    }}
                    placeholder={t.drawer.scopePlaceholder}
                    className="h-7 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 font-mono-tabular text-[12px] text-[var(--fg)]"
                  />
                </div>
              )}

              <div className="col-span-2 flex flex-col gap-1.5">
                <span className="text-[11px] font-medium text-[var(--fg-3)]">
                  {t.drawer.modules}
                </span>
                <div className="flex flex-wrap items-center gap-1.5">
                  {modChoices.map((k) => {
                    const m = resolveModule(k, prefix, locale, moduleAliases);
                    const active = ownMods.has(k);
                    return (
                      <button
                        key={k}
                        type="button"
                        disabled={busy}
                        onClick={() => void handleToggleMod(k)}
                        className={`inline-flex h-6 items-center gap-1.5 rounded-[var(--radius-xs)] border px-2 text-[11.5px] transition-colors cursor-pointer ${
                          active
                            ? "border-[var(--accent-border)] bg-[var(--accent-soft)] font-medium text-[var(--accent)]"
                            : "border-[var(--border)] bg-[var(--surface)] text-[var(--fg-2)] hover:border-[var(--border-strong)] hover:text-[var(--fg)]"
                        }`}
                      >
                        <span
                          className="h-1.5 w-1.5 rounded-full"
                          style={{ backgroundColor: m.color }}
                        />
                        <span>{m.label}</span>
                      </button>
                    );
                  })}
                  {newModOpen ? (
                    <div className="inline-flex items-center gap-1">
                      <input
                        type="text"
                        value={newModDraft}
                        onChange={(e) => setNewModDraft(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") void handleAddCustomMod();
                          if (e.key === "Escape") setNewModOpen(false);
                        }}
                        placeholder={t.drawer.newModulePrompt}
                        className="h-6 w-28 rounded-[var(--radius-xs)] border border-[var(--border-strong)] bg-[var(--surface)] px-1.5 font-mono-tabular text-[11px]"
                      />
                      <Button size="sm" variant="primary" onClick={() => void handleAddCustomMod()}>
                        {t.actions.add}
                      </Button>
                    </div>
                  ) : (
                    <button
                      type="button"
                      onClick={() => setNewModOpen(true)}
                      className="inline-flex h-6 items-center rounded-[var(--radius-xs)] border border-dashed border-[var(--border-strong)] px-2 text-[11.5px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                    >
                      {t.drawer.addModule}
                    </button>
                  )}
                </div>
              </div>
            </div>
          </section>

          {/* Comments */}
          <section className="space-y-2">
            <div className="text-[11.5px] font-semibold text-[var(--fg-3)]">
              {t.drawer.comments} ({comments.length})
            </div>
            {comments.length === 0 ? (
              <div className="text-[12px] text-[var(--fg-3)]">{t.drawer.noComments}</div>
            ) : (
              <div className="space-y-1.5">
                {comments.map((c) => (
                  <div
                    key={c.id}
                    className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-2.5 text-[12.5px]"
                  >
                    <div className="mb-1 flex justify-between font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                      <span>{c.author}</span>
                      <span>{c.createdAt.replace("T", " ").slice(0, 16)}</span>
                    </div>
                    <div className="whitespace-pre-wrap break-words select-text">{c.text}</div>
                  </div>
                ))}
              </div>
            )}

            <div className="flex items-start gap-2">
              <textarea
                rows={2}
                value={commentDraft}
                onChange={(e) => setCommentDraft(e.target.value)}
                onKeyDown={(e) => {
                  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
                    e.preventDefault();
                    void handleSendComment();
                  }
                }}
                placeholder={t.drawer.commentPlaceholder}
                className="min-h-[38px] flex-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2.5 py-1.5 text-[12.5px] focus:border-[var(--border-strong)]"
              />
              <Button
                variant="primary"
                disabled={!commentDraft.trim() || busy}
                onClick={() => void handleSendComment()}
                className="h-9"
              >
                {t.actions.send}
              </Button>
            </div>
          </section>
        </div>

        {/* Drawer Footer */}
        <footer className="flex shrink-0 items-center justify-between border-t border-[var(--border)] bg-[var(--surface-subtle)] px-5 py-2.5">
          <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">{issue.id}</span>
          {issue.status === "closed" ? (
            <Button variant="outline" disabled={busy} onClick={() => void handleReopen()}>
              {t.actions.reopen}
            </Button>
          ) : (
            <Button variant="accent" disabled={busy} onClick={() => onRequestClose(issue)}>
              <Check className="h-3.5 w-3.5" />
              <span>{t.actions.markDone}</span>
            </Button>
          )}
        </footer>
      </aside>
    </>
  );
}
