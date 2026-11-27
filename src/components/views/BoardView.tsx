import { Check, ChevronDown, ChevronLeft, ChevronRight, Play, Plus, Sparkles } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { AssigneeDot, ModuleInlineList, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { extractPlainSummary, isClosedIterationRound, shortParentLabel } from "@/lib/human-context";
import { formatIpcError } from "@/lib/ipc";
import { buildAgentPrompt } from "@/lib/prompt-builder";
import { useRadarStore } from "@/stores/useRadarStore";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { BoardLane, IssueDto, ModuleAliasConfig } from "@/types/ipc";

const LANES: readonly BoardLane[] = ["in_progress", "ready", "blocked", "closed"];

export interface BoardViewProps {
  issues: IssueDto[];
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
  onRequestClose: (issue: IssueDto) => void;
}

export function BoardView({ issues, modulePrefix, moduleAliases, onRequestClose }: BoardViewProps) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const { snapshot, byId, createIssue, updateIssue } = useWorkspaceStore();
  const badgeByIssueId = useRadarStore((s) => s.badgeByIssueId);
  const {
    activeModule,
    boardGroupMode,
    hideClosedRounds,
    closedLaneCollapsed,
    setHideClosedRounds,
    toggleClosedLane,
    openDrawer,
    pushToast,
  } = useUiStore();

  const [dragId, setDragId] = useState<string | null>(null);
  const [overTarget, setOverTarget] = useState<string | null>(null);
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});
  const [quickAddLane, setQuickAddLane] = useState<"in_progress" | "ready" | null>(null);
  const [quickAddTitle, setQuickAddTitle] = useState("");
  const [quickAddBusy, setQuickAddBusy] = useState(false);
  const quickAddInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (quickAddLane) {
      quickAddInputRef.current?.focus();
    }
  }, [quickAddLane]);

  const handleDrop = async (lane: BoardLane) => {
    setOverTarget(null);
    const id = dragId;
    setDragId(null);
    if (!id) return;
    const issue = byId[id];
    if (!issue || issue.lane === lane) return;
    if (lane === "closed") {
      onRequestClose(issue);
      return;
    }
    const nextStatus = lane === "ready" ? "open" : lane;
    try {
      await updateIssue({ issueId: id, status: nextStatus });
      pushToast(`${issue.shortId} → ${t.lanes[lane]}`, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handleQuickStart = async (it: IssueDto) => {
    try {
      await updateIssue({ issueId: it.id, status: "in_progress" });
      pushToast(`${it.shortId} → ${t.lanes.in_progress}`, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handleCopyPrompt = async (it: IssueDto) => {
    if (!snapshot) return;
    const prompt = buildAgentPrompt(it, snapshot, byId, locale);
    await navigator.clipboard?.writeText(prompt);
    pushToast(t.drawer.copiedPrompt, "ok");
  };

  const handleQuickAddSubmit = async (lane: "in_progress" | "ready") => {
    const clean = quickAddTitle.trim();
    if (!clean || quickAddBusy) return;
    setQuickAddBusy(true);
    try {
      const id = await createIssue({
        title: clean,
        issueType: "task",
        priority: 1,
        status: lane === "in_progress" ? "in_progress" : "open",
        labels: activeModule ? [activeModule] : [],
        blockedBy: [],
      });
      setQuickAddTitle("");
      pushToast(`${t.create.createdToast}: ${id ?? clean}`, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setQuickAddBusy(false);
    }
  };

  const renderCard = (it: IssueDto, isClosed: boolean, showParent: boolean) => {
    // Completed cards use a quiet, compact layout so active tasks stay visually dominant
    if (isClosed) {
      return (
        <article
          key={it.id}
          draggable
          onDragStart={(e) => {
            setDragId(it.id);
            e.dataTransfer.effectAllowed = "move";
          }}
          onDragEnd={() => {
            setDragId(null);
            setOverTarget(null);
          }}
          onClick={() => openDrawer(it.id)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              openDrawer(it.id);
            }
          }}
          className={`group min-w-0 overflow-hidden rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-transparent px-2.5 py-2 transition-colors hover:border-[var(--border-strong)] hover:bg-[var(--surface)] cursor-pointer ${
            dragId === it.id ? "opacity-40" : ""
          }`}
        >
          <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-[var(--fg-3)]">
            <StatusRing lane="closed" size={11} />
            <span className="shrink-0 whitespace-nowrap font-mono-tabular">{it.shortId}</span>
            {it.issueType === "bug" && (
              <span className="shrink-0 whitespace-nowrap text-[var(--danger)]/80">
                {t.types.bug}
              </span>
            )}
            <div className="ml-auto flex min-w-0 justify-end">
              <AssigneeDot name={it.assignee} />
            </div>
          </div>
          <div className="mt-1 line-clamp-2 break-words text-[12.5px] leading-[1.4] text-[var(--fg-2)] group-hover:text-[var(--fg)]">
            {it.title}
          </div>
        </article>
      );
    }

    const openBlockers = it.openBlockedBy
      .map((bid) => byId[bid])
      .filter((x): x is IssueDto => Boolean(x));
    const unlocksCount = it.blocks.filter((uid) => byId[uid]?.status !== "closed").length;
    const hasQuickStart = it.lane === "ready";
    const hasQuickDone = it.lane === "in_progress";
    const parentReadable =
      showParent && it.parentId && it.parentTitle && it.issueType !== "epic"
        ? shortParentLabel(it.parentTitle, 18)
        : null;
    const runtimeBadge = badgeByIssueId[it.id];
    const showFoot =
      Boolean(runtimeBadge) ||
      it.modules.length > 0 ||
      openBlockers.length > 0 ||
      (it.lane === "ready" && unlocksCount > 0) ||
      Boolean(parentReadable);

    return (
      <article
        key={it.id}
        draggable
        onDragStart={(e) => {
          setDragId(it.id);
          e.dataTransfer.effectAllowed = "move";
        }}
        onDragEnd={() => {
          setDragId(null);
          setOverTarget(null);
        }}
        onClick={() => openDrawer(it.id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            openDrawer(it.id);
          }
        }}
        className={`group relative min-w-0 overflow-hidden rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-[var(--card-pad-x)] py-[var(--card-pad-y)] transition-[border-color,background-color,box-shadow] duration-100 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-sm)] cursor-pointer ${
          dragId === it.id ? "opacity-40" : ""
        }`}
      >
        {/* Row 1: Priority glyph + mono ID + optional Epic/Bug + #N + right slot (assignee -> 1-click hover actions) */}
        <div className="mb-1.5 flex min-h-[20px] min-w-0 items-center gap-1.5">
          <PriorityGlyph priority={it.priority} />
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              void navigator.clipboard?.writeText(it.id);
              pushToast(`${t.drawer.copiedId}: ${it.id}`, "ok");
            }}
            title={`${t.actions.copyId}: ${it.id}`}
            className="shrink-0 whitespace-nowrap font-mono-tabular text-[11px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
          >
            {it.shortId}
          </button>
          {it.issueType === "epic" && (
            <span className="shrink-0 whitespace-nowrap text-[11px] text-[var(--fg-3)]">
              {t.types.epic}
            </span>
          )}
          {it.issueType === "bug" && (
            <span className="shrink-0 whitespace-nowrap text-[11px] text-[var(--danger)]">
              {t.types.bug}
            </span>
          )}
          {it.smart.githubNumber && (
            <span className="shrink-0 whitespace-nowrap font-mono-tabular text-[11px] text-[var(--fg-3)]">
              #{it.smart.githubNumber}
            </span>
          )}

          <div className="relative ml-auto flex min-h-[20px] min-w-0 flex-1 items-center justify-end">
            <div className="flex min-w-0 max-w-full justify-end transition-opacity duration-100 group-hover:opacity-0">
              <AssigneeDot name={it.assignee} />
            </div>

            {/* 1-click hover quick action bar */}
            <div className="pointer-events-none absolute top-1/2 right-0 flex -translate-y-1/2 items-center gap-1 opacity-0 transition-opacity duration-100 group-hover:pointer-events-auto group-hover:opacity-100">
              <button
                type="button"
                title={t.board.copyAiBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  void handleCopyPrompt(it);
                }}
                className="inline-flex h-5 items-center gap-0.5 whitespace-nowrap rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-1.5 text-[10.5px] text-[var(--fg-2)] hover:border-[var(--accent-border)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent)] cursor-pointer"
              >
                <Sparkles className="h-2.5 w-2.5 shrink-0" />
                <span>AI</span>
              </button>
              {hasQuickStart && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void handleQuickStart(it);
                  }}
                  className="inline-flex h-5 items-center gap-0.5 whitespace-nowrap rounded-[var(--radius-xs)] border border-[var(--accent-border)] bg-[var(--accent-soft)] px-1.5 text-[10.5px] font-medium text-[var(--accent)] hover:brightness-105 cursor-pointer"
                >
                  <Play className="h-2.5 w-2.5 shrink-0" />
                  <span>{t.board.startBtn}</span>
                </button>
              )}
              {hasQuickDone && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRequestClose(it);
                  }}
                  className="inline-flex h-5 items-center gap-0.5 whitespace-nowrap rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-hover)] px-1.5 text-[10.5px] text-[var(--fg-2)] hover:border-[var(--success)] hover:bg-[var(--success-soft)] hover:text-[var(--success)] cursor-pointer"
                >
                  <Check className="h-3 w-3 shrink-0" />
                  <span>{t.actions.done}</span>
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Row 2: Crisp readable title */}
        <div className="line-clamp-3 break-words text-[13px] font-medium leading-[1.45] text-[var(--fg)]">
          {it.title}
        </div>

        {/* Row 3: Minimal high-signal footer */}
        {showFoot && (
          <div className="mt-2 flex min-w-0 flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] text-[var(--fg-3)]">
            {runtimeBadge && (
              <span
                className={`inline-flex max-w-full items-center gap-1 whitespace-nowrap rounded-[var(--radius-xs)] px-1.5 py-0.25 text-[10.5px] font-medium ${
                  runtimeBadge.kind === "waiting"
                    ? "bg-[var(--accent-soft)] text-[var(--accent)]"
                    : runtimeBadge.kind === "conflict"
                      ? "bg-[var(--danger)]/10 text-[var(--danger)]"
                      : runtimeBadge.kind === "working"
                        ? "bg-[var(--success-soft)] text-[var(--success)]"
                        : "bg-[var(--surface-subtle)] text-[var(--fg-3)]"
                }`}
              >
                <span
                  className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                    runtimeBadge.kind === "waiting"
                      ? "bg-[var(--accent)]"
                      : runtimeBadge.kind === "conflict"
                        ? "bg-[var(--danger)]"
                        : runtimeBadge.kind === "working"
                          ? "bg-[var(--success)]"
                          : "bg-[var(--fg-3)]"
                  }`}
                />
                <span className="truncate">
                  {runtimeBadge.kind === "waiting"
                    ? `${t.board.badgeWaiting} · ${runtimeBadge.label}`
                    : runtimeBadge.kind === "conflict"
                      ? t.board.badgeConflict
                      : runtimeBadge.kind === "stale"
                        ? t.board.badgeStale
                        : runtimeBadge.label}
                </span>
              </span>
            )}
            <ModuleInlineList
              modules={it.modules}
              prefix={modulePrefix}
              locale={locale}
              aliases={moduleAliases}
              limit={2}
            />

            {openBlockers.length > 0 && openBlockers[0] ? (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  openDrawer(openBlockers[0]?.id ?? it.id);
                }}
                className="shrink-0 whitespace-nowrap font-medium text-[11px] text-[var(--danger)] hover:underline underline-offset-2 cursor-pointer"
              >
                {t.board.blockedByPrefix} {openBlockers[0].shortId}
                {openBlockers.length > 1 ? ` +${openBlockers.length - 1}` : ""}
              </button>
            ) : it.lane === "ready" && unlocksCount > 0 ? (
              <span className="shrink-0 whitespace-nowrap text-[11px] text-[var(--success)]">
                {t.board.unlocksCount} {unlocksCount} {t.board.itemsUnit}
              </span>
            ) : null}

            {openBlockers.length === 0 && parentReadable && it.parentId && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  openDrawer(it.parentId ?? it.id);
                }}
                title={it.parentTitle ?? undefined}
                className="max-w-full truncate text-[11px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
              >
                {t.board.belongsTo} {parentReadable}
              </button>
            )}
          </div>
        )}
      </article>
    );
  };

  // Swimlane grouping by Parent Epic
  const epicGroups = useMemo(() => {
    if (boardGroupMode !== "epic") return [];
    const map = new Map<string, { key: string; epic: IssueDto | null; items: IssueDto[] }>();
    for (const it of issues) {
      if (it.issueType === "epic") continue;
      const gKey = it.parentId ?? "__standalone__";
      const existing = map.get(gKey);
      if (existing) {
        existing.items.push(it);
      } else {
        map.set(gKey, {
          key: gKey,
          epic: it.parentId ? (byId[it.parentId] ?? null) : null,
          items: [it],
        });
      }
    }
    const groups = [...map.values()];
    groups.sort((a, b) => {
      const aActive = a.items.some((x) => x.lane !== "closed") ? 0 : 1;
      const bActive = b.items.some((x) => x.lane !== "closed") ? 0 : 1;
      if (aActive !== bActive) return aActive - bActive;
      const ap = a.epic?.priority ?? 9;
      const bp = b.epic?.priority ?? 9;
      return ap - bp || b.items.length - a.items.length;
    });
    return groups;
  }, [boardGroupMode, issues, byId]);

  if (boardGroupMode === "epic") {
    return (
      <div className="min-h-0 flex-1 overflow-y-auto">
        {epicGroups.map((group) => {
          const isCollapsed = Boolean(collapsedGroups[group.key]);
          const doneCnt = group.items.filter((x) => x.lane === "closed").length;
          const summary = group.epic ? extractPlainSummary(group.epic.description, 110) : "";

          return (
            <section key={group.key} className="border-b border-[var(--border)]">
              <div className="flex items-center gap-2 bg-[var(--surface-subtle)] px-4 py-2.5">
                <button
                  type="button"
                  onClick={() =>
                    setCollapsedGroups((prev) => ({ ...prev, [group.key]: !prev[group.key] }))
                  }
                  className="text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                >
                  {isCollapsed ? (
                    <ChevronRight className="h-3.5 w-3.5" />
                  ) : (
                    <ChevronDown className="h-3.5 w-3.5" />
                  )}
                </button>
                {group.epic && <StatusRing lane={group.epic.lane} />}
                {group.epic ? (
                  <button
                    type="button"
                    onClick={() => openDrawer(group.epic?.id ?? "")}
                    className="min-w-0 truncate text-left text-[13px] font-semibold text-[var(--fg)] hover:text-[var(--accent)] cursor-pointer"
                  >
                    {group.epic.title}
                  </button>
                ) : (
                  <span className="text-[13px] font-semibold text-[var(--fg-2)]">
                    {t.board.standaloneGroup}
                  </span>
                )}
                {group.epic && (
                  <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                    {group.epic.shortId}
                  </span>
                )}
                <span className="ml-auto shrink-0 font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                  {doneCnt} / {group.items.length}
                </span>
              </div>

              {summary && !isCollapsed && (
                <div className="border-t border-[var(--border-subtle)] bg-[var(--surface-subtle)] px-9 pb-2 text-[11.5px] text-[var(--fg-3)]">
                  {summary}
                </div>
              )}

              {!isCollapsed && (
                <div className="grid grid-cols-[repeat(4,minmax(220px,1fr))] overflow-x-auto border-t border-[var(--border-subtle)]">
                  {LANES.map((lane) => {
                    const laneCardsAll = group.items.filter((x) => x.lane === lane);
                    const hiddenRounds =
                      lane === "closed" && hideClosedRounds
                        ? laneCardsAll.filter(isClosedIterationRound).length
                        : 0;
                    const laneCards =
                      lane === "closed" && hideClosedRounds
                        ? laneCardsAll.filter((x) => !isClosedIterationRound(x))
                        : laneCardsAll;
                    const dropKey = `${group.key}:${lane}`;

                    return (
                      <div
                        key={lane}
                        role="listbox"
                        aria-label={t.lanes[lane]}
                        data-lane={lane}
                        onDragOver={(e) => {
                          e.preventDefault();
                          if (overTarget !== dropKey) setOverTarget(dropKey);
                        }}
                        onDragLeave={() => {
                          if (overTarget === dropKey) setOverTarget(null);
                        }}
                        onDrop={(e) => {
                          e.preventDefault();
                          void handleDrop(lane);
                        }}
                        className={`flex min-w-0 flex-col gap-[var(--lane-gap)] border-r border-[var(--border-subtle)] p-2.5 last:border-r-0 ${
                          overTarget === dropKey ? "bg-[var(--accent-soft)]" : ""
                        }`}
                      >
                        <div className="flex items-center gap-1.5 whitespace-nowrap px-1 pb-0.5 text-[11px] text-[var(--fg-3)]">
                          <StatusRing lane={lane} size={11} />
                          <span>{t.lanes[lane]}</span>
                          <span className="font-mono-tabular">{laneCardsAll.length}</span>
                        </div>
                        {laneCards.map((it) => renderCard(it, lane === "closed", false))}
                        {hiddenRounds > 0 && (
                          <button
                            type="button"
                            onClick={() => setHideClosedRounds(false)}
                            className="py-1.5 text-center text-[11px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                          >
                            +{hiddenRounds} {t.board.hiddenRoundsHint} · {t.board.showRounds}
                          </button>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </section>
          );
        })}
      </div>
    );
  }

  // Flat 4-column hairline Kanban with collapsible Closed rail & inline Quick Add
  return (
    <div
      className={`grid min-h-0 flex-1 overflow-x-auto overflow-y-hidden ${
        closedLaneCollapsed
          ? "grid-cols-[repeat(3,minmax(220px,1fr))_44px]"
          : "grid-cols-[repeat(4,minmax(220px,1fr))]"
      }`}
    >
      {LANES.map((lane) => {
        const allCards = issues.filter((it) => it.lane === lane);
        const hiddenRounds =
          lane === "closed" && hideClosedRounds
            ? allCards.filter(isClosedIterationRound).length
            : 0;
        const cards =
          lane === "closed" && hideClosedRounds
            ? allCards.filter((it) => !isClosedIterationRound(it))
            : allCards;
        const isClosed = lane === "closed";
        const isDragOver = overTarget === lane;
        const canQuickAdd = lane === "in_progress" || lane === "ready";

        if (isClosed && closedLaneCollapsed) {
          return (
            <button
              key={lane}
              type="button"
              onClick={toggleClosedLane}
              title={t.board.expandClosed}
              onDragOver={(e) => {
                e.preventDefault();
                if (overTarget !== lane) setOverTarget(lane);
              }}
              onDragLeave={() => {
                if (overTarget === lane) setOverTarget(null);
              }}
              onDrop={(e) => {
                e.preventDefault();
                void handleDrop(lane);
              }}
              className={`flex min-h-0 flex-col items-center gap-2.5 border-l border-[var(--border-subtle)] bg-[var(--surface-subtle)]/50 py-3 text-[var(--fg-3)] transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer ${
                isDragOver ? "bg-[var(--accent-soft)]" : ""
              }`}
            >
              <ChevronLeft className="h-3.5 w-3.5 shrink-0" />
              <StatusRing lane="closed" />
              <span className="font-mono-tabular text-[11px]">{allCards.length}</span>
              <span
                className="text-[11.5px] font-medium tracking-wider"
                style={{ writingMode: "vertical-rl" }}
              >
                {t.lanes.closed}
              </span>
            </button>
          );
        }

        return (
          <div
            key={lane}
            role="listbox"
            aria-label={t.lanes[lane]}
            data-lane={lane}
            onDragOver={(e) => {
              e.preventDefault();
              if (overTarget !== lane) setOverTarget(lane);
            }}
            onDragLeave={() => {
              if (overTarget === lane) setOverTarget(null);
            }}
            onDrop={(e) => {
              e.preventDefault();
              void handleDrop(lane);
            }}
            className={`flex min-h-0 min-w-0 flex-col border-r border-[var(--border-subtle)] last:border-r-0 transition-colors ${
              isClosed ? "bg-[var(--surface-subtle)]/35" : ""
            } ${isDragOver ? "bg-[var(--accent-soft)]" : ""}`}
          >
            {/* Flat hairline column header with subtle quick-add or collapse trigger */}
            <header className="flex shrink-0 items-center gap-2 whitespace-nowrap px-3 pt-3 pb-2">
              <StatusRing lane={lane} />
              <h2 className="text-[12.5px] font-semibold text-[var(--fg)]">{t.lanes[lane]}</h2>
              <span className="font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                {allCards.length}
              </span>
              {canQuickAdd && (
                <button
                  type="button"
                  title={t.board.quickAdd}
                  onClick={() => {
                    setQuickAddLane(lane);
                    setQuickAddTitle("");
                  }}
                  className="ml-auto grid h-5 w-5 place-items-center rounded-[var(--radius-xs)] text-[var(--fg-3)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
                >
                  <Plus className="h-3.5 w-3.5" />
                </button>
              )}
              {isClosed && (
                <button
                  type="button"
                  title={t.board.collapseClosed}
                  onClick={toggleClosedLane}
                  className="ml-auto grid h-5 w-5 place-items-center rounded-[var(--radius-xs)] text-[var(--fg-3)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
                >
                  <ChevronRight className="h-3.5 w-3.5" />
                </button>
              )}
            </header>

            <div className="flex min-h-0 flex-1 flex-col gap-[var(--lane-gap)] overflow-y-auto px-2.5 pt-1 pb-4">
              {cards.length === 0 && hiddenRounds === 0 && quickAddLane !== lane ? (
                <div className="py-6 text-center text-[12px] text-[var(--fg-3)]">
                  {t.laneEmpty[lane]}
                </div>
              ) : (
                <>
                  {cards.map((it) => renderCard(it, isClosed, true))}
                  {hiddenRounds > 0 && (
                    <button
                      type="button"
                      onClick={() => setHideClosedRounds(false)}
                      className="mt-1 rounded-[var(--radius-sm)] border border-dashed border-[var(--border)] py-2 text-center text-[11.5px] text-[var(--fg-3)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] cursor-pointer"
                    >
                      +{hiddenRounds} {t.board.hiddenRoundsHint} · {t.board.showRounds}
                    </button>
                  )}
                </>
              )}

              {/* Inline quick-add right inside active lanes */}
              {canQuickAdd &&
                (quickAddLane === lane ? (
                  <div className="rounded-[var(--radius-md)] border border-[var(--accent-border)] bg-[var(--surface)] p-2 shadow-[var(--shadow-xs)]">
                    <input
                      ref={quickAddInputRef}
                      type="text"
                      value={quickAddTitle}
                      disabled={quickAddBusy}
                      onChange={(e) => setQuickAddTitle(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.preventDefault();
                          void handleQuickAddSubmit(lane);
                        } else if (e.key === "Escape") {
                          setQuickAddLane(null);
                          setQuickAddTitle("");
                        }
                      }}
                      onBlur={() => {
                        if (!quickAddTitle.trim()) {
                          setQuickAddLane(null);
                        }
                      }}
                      placeholder={t.board.quickAddPlaceholder}
                      className="w-full bg-transparent text-[12.5px] text-[var(--fg)] outline-none placeholder:text-[var(--fg-3)]"
                    />
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => {
                      setQuickAddLane(lane);
                      setQuickAddTitle("");
                    }}
                    className="mt-0.5 flex items-center gap-1.5 rounded-[var(--radius-sm)] px-2 py-1.5 text-[11.5px] text-[var(--fg-3)] transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--fg-2)] cursor-pointer"
                  >
                    <Plus className="h-3.5 w-3.5 shrink-0" />
                    <span>{t.board.quickAdd}</span>
                  </button>
                ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
