import {
  ArrowRight,
  Check,
  ChevronDown,
  ChevronRight,
  HelpCircle,
  Play,
  Plus,
  Sparkles,
  Zap,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { IssueDetailPanel } from "@/components/drawer/IssueDrawer";
import { Button } from "@/components/ui/Button";
import { AssigneeDot, ModuleInlineList, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { isClosedIterationRound } from "@/lib/human-context";
import { formatIpcError } from "@/lib/ipc";
import { buildAgentPrompt } from "@/lib/prompt-builder";
import { sameWorkspacePath, useRadarStore } from "@/stores/useRadarStore";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { type TasksFilter, type TasksGroupBy, useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { BoardLane, IssueDto, ModuleAliasConfig } from "@/types/ipc";

export interface TasksViewProps {
  issues: IssueDto[];
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
  isWide: boolean;
  onRequestClose: (issue: IssueDto) => void;
}

interface TaskGroup {
  key: string;
  label: string;
  lane?: BoardLane;
  progress?: string;
  items: IssueDto[];
}

const LANE_ORDER: readonly BoardLane[] = ["in_progress", "ready", "blocked", "closed"];

export function TasksView({
  issues,
  modulePrefix,
  moduleAliases,
  isWide,
  onRequestClose,
}: TasksViewProps) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const { activePath, snapshot, byId, createIssue, updateIssue } = useWorkspaceStore();
  const rawRadar = useRadarStore((s) => s.radar);
  const radar = rawRadar && sameWorkspacePath(rawRadar.repoPath, activePath) ? rawRadar : null;
  const badgeByIssueId = useRadarStore((s) => s.badgeByIssueId);

  const {
    activeModule,
    tasksFilter,
    tasksGroupBy,
    hideClosedRounds,
    selectedIssueId,
    paletteOpen,
    settingsOpen,
    createModalOpen,
    setTasksFilter,
    setTasksGroupBy,
    setHideClosedRounds,
    setActiveView,
    openDrawer,
    closeDrawer,
    pushToast,
  } = useUiStore();

  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});
  const [splitDismissed, setSplitDismissed] = useState(false);
  const [quickTitle, setQuickTitle] = useState("");
  const [quickBusy, setQuickBusy] = useState(false);
  const rowRefs = useRef<Record<string, HTMLDivElement | null>>({});

  // Work primarily with actionable tasks (exclude epics unless specifically searching or viewing all)
  const taskItems = useMemo(() => issues.filter((it) => it.issueType !== "epic"), [issues]);

  const counts = useMemo(() => {
    let inProgress = 0;
    let ready = 0;
    let blocked = 0;
    let closed = 0;
    for (const it of taskItems) {
      if (it.lane === "in_progress") inProgress += 1;
      else if (it.lane === "ready") ready += 1;
      else if (it.lane === "blocked") blocked += 1;
      else closed += 1;
    }
    return {
      open: inProgress + ready + blocked,
      in_progress: inProgress,
      ready,
      blocked,
      closed,
      all: taskItems.length,
    };
  }, [taskItems]);

  const { visibleItems, hiddenClosedRoundsCount } = useMemo(() => {
    const filtered = taskItems.filter((it) => {
      if (tasksFilter === "open") return it.lane !== "closed";
      if (tasksFilter === "all") return true;
      return it.lane === tasksFilter;
    });

    let folded = 0;
    const afterRoundFold = filtered.filter((it) => {
      if (hideClosedRounds && it.lane === "closed" && isClosedIterationRound(it)) {
        folded += 1;
        return false;
      }
      return true;
    });

    // Sort: active lanes first -> priority ascending -> downstream unlock impact descending -> updated desc
    const laneRank = (lane: BoardLane) => LANE_ORDER.indexOf(lane);
    const sorted = [...afterRoundFold].sort((a, b) => {
      const lr = laneRank(a.lane) - laneRank(b.lane);
      if (lr !== 0) return lr;
      if (a.priority !== b.priority) return a.priority - b.priority;
      const aUnlocks = a.blocks.filter((id) => byId[id]?.status !== "closed").length;
      const bUnlocks = b.blocks.filter((id) => byId[id]?.status !== "closed").length;
      if (aUnlocks !== bUnlocks) return bUnlocks - aUnlocks;
      return b.updatedAt.localeCompare(a.updatedAt);
    });

    return { visibleItems: sorted, hiddenClosedRoundsCount: folded };
  }, [taskItems, tasksFilter, hideClosedRounds, byId]);

  const groups = useMemo<TaskGroup[]>(() => {
    if (visibleItems.length === 0) return [];

    if (tasksGroupBy === "priority") {
      const buckets = new Map<number, IssueDto[]>();
      for (const it of visibleItems) {
        const list = buckets.get(it.priority) ?? [];
        list.push(it);
        buckets.set(it.priority, list);
      }
      return [...buckets.entries()]
        .sort(([a], [b]) => a - b)
        .map(([prio, items]) => ({
          key: `prio:${prio}`,
          label: `P${prio}`,
          items,
        }));
    }

    if (tasksGroupBy === "epic") {
      const buckets = new Map<string, IssueDto[]>();
      for (const it of visibleItems) {
        const key = it.parentId ?? "__standalone__";
        const list = buckets.get(key) ?? [];
        list.push(it);
        buckets.set(key, list);
      }
      return [...buckets.entries()]
        .map(([key, items]) => {
          const ep = key === "__standalone__" ? null : (byId[key] ?? null);
          const allKids = ep ? ep.childrenIds.map((id) => byId[id]).filter(Boolean) : [];
          const doneKids = allKids.filter((k) => k?.lane === "closed").length;
          return {
            key: `epic:${key}`,
            label: ep ? `${ep.shortId} · ${ep.title}` : t.board.standaloneGroup,
            lane: ep?.lane,
            progress: ep && allKids.length > 0 ? `${doneKids}/${allKids.length}` : undefined,
            items,
          };
        })
        .sort((a, b) => {
          if (a.key === "epic:__standalone__") return 1;
          if (b.key === "epic:__standalone__") return -1;
          return b.items.length - a.items.length;
        });
    }

    // Default: Group by Status (skip empty lanes so zero dead space!)
    const out: TaskGroup[] = [];
    for (const lane of LANE_ORDER) {
      const items = visibleItems.filter((it) => it.lane === lane);
      if (items.length === 0) continue;
      out.push({
        key: `lane:${lane}`,
        label: t.lanes[lane],
        lane,
        items,
      });
    }
    return out;
  }, [visibleItems, tasksGroupBy, byId, t]);

  // Flat ordered list of currently expanded visible items for ↑ / ↓ keyboard navigation
  const navigableItems = useMemo(() => {
    const flat: IssueDto[] = [];
    for (const g of groups) {
      if (collapsedGroups[g.key]) continue;
      flat.push(...g.items);
    }
    return flat;
  }, [groups, collapsedGroups]);

  // Resolve which issue is shown in the right split-pane preview
  const splitIssueId = useMemo(() => {
    if (!isWide || splitDismissed) return null;
    if (selectedIssueId && byId[selectedIssueId]) return selectedIssueId;
    return navigableItems[0]?.id ?? null;
  }, [isWide, splitDismissed, selectedIssueId, byId, navigableItems]);

  // Keyboard ↑ / ↓ / J / K navigation through the task stream
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (paletteOpen || settingsOpen || createModalOpen) return;
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase() ?? "";
      if (
        tag === "input" ||
        tag === "textarea" ||
        tag === "select" ||
        target?.isContentEditable === true
      ) {
        return;
      }
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (navigableItems.length === 0) return;

      const isDown = e.key === "ArrowDown" || e.key === "j";
      const isUp = e.key === "ArrowUp" || e.key === "k";
      if (!isDown && !isUp) return;

      e.preventDefault();
      const curId = selectedIssueId ?? splitIssueId;
      const curIdx = navigableItems.findIndex((x) => x.id === curId);
      const nextIdx = isDown
        ? curIdx < 0
          ? 0
          : Math.min(navigableItems.length - 1, curIdx + 1)
        : curIdx <= 0
          ? 0
          : curIdx - 1;
      const next = navigableItems[nextIdx];
      if (next) {
        setSplitDismissed(false);
        openDrawer(next.id);
        rowRefs.current[next.id]?.scrollIntoView({ block: "nearest" });
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [
    navigableItems,
    selectedIssueId,
    splitIssueId,
    paletteOpen,
    settingsOpen,
    createModalOpen,
    openDrawer,
  ]);

  const waitingAlert = useMemo(
    () => radar?.attention.find((a) => a.kind === "waiting-human") ?? null,
    [radar],
  );

  const recommendedBlocker = useMemo(() => {
    if (tasksFilter !== "open") return null;
    const top = radar?.topBlockers.find((b) => b.actionable && b.directUnblocks > 0);
    if (!top) return null;
    const issue = byId[top.issueId];
    if (!issue || issue.lane === "closed") return null;
    return { hotspot: top, issue };
  }, [radar, tasksFilter, byId]);

  const handleQuickCreate = async () => {
    const clean = quickTitle.trim();
    if (!clean || quickBusy) return;
    setQuickBusy(true);
    try {
      const targetStatus = tasksFilter === "in_progress" ? "in_progress" : "open";
      const id = await createIssue({
        title: clean,
        issueType: "task",
        priority: 1,
        status: targetStatus,
        labels: activeModule ? [activeModule] : [],
        blockedBy: [],
      });
      setQuickTitle("");
      pushToast(`${t.create.createdToast}: ${id ?? clean}`, "ok");
      if (id) {
        if (tasksFilter === "closed") setTasksFilter("open");
        setSplitDismissed(false);
        openDrawer(id);
      }
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setQuickBusy(false);
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

  const filterTabs: readonly { id: TasksFilter; label: string; count: number }[] = [
    { id: "open", label: t.board.tabOpen, count: counts.open },
    { id: "in_progress", label: t.lanes.in_progress, count: counts.in_progress },
    { id: "ready", label: t.lanes.ready, count: counts.ready },
    { id: "blocked", label: t.lanes.blocked, count: counts.blocked },
    { id: "closed", label: t.lanes.closed, count: counts.closed },
  ];

  const groupChoices: readonly { id: TasksGroupBy; label: string }[] = [
    { id: "status", label: t.board.groupByStatus },
    { id: "epic", label: t.board.groupByEpic },
    { id: "priority", label: t.board.groupByPriority },
  ];

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden">
      {/* Left Master Pane: Clean Scannable Task Stream */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        {/* Focus Filter Tabs + Group Switcher */}
        <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[var(--border)] bg-[var(--bg)] px-4 py-2">
          <div className="flex flex-wrap items-center gap-1">
            {filterTabs.map((tab) => {
              const active = tasksFilter === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setTasksFilter(tab.id)}
                  className={`inline-flex h-7 items-center gap-1.5 whitespace-nowrap rounded-[var(--radius-sm)] px-2.5 text-[12px] transition-colors cursor-pointer ${
                    active
                      ? "bg-[var(--surface-active)] font-medium text-[var(--fg)]"
                      : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
                  }`}
                >
                  <span>{tab.label}</span>
                  <span
                    className={`font-mono-tabular text-[11px] ${
                      active ? "text-[var(--fg-2)]" : "text-[var(--fg-3)]"
                    }`}
                  >
                    {tab.count}
                  </span>
                </button>
              );
            })}
          </div>

          <div className="ml-auto inline-flex h-7 shrink-0 items-center rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
            {groupChoices.map((g) => (
              <button
                key={g.id}
                type="button"
                onClick={() => setTasksGroupBy(g.id)}
                className={`h-[22px] shrink-0 whitespace-nowrap rounded-[var(--radius-xs)] px-2 text-[11px] transition-colors cursor-pointer ${
                  tasksGroupBy === g.id
                    ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                    : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                }`}
              >
                {g.label}
              </button>
            ))}
          </div>
        </div>

        {/* Contextual Next-Action Banner (Only when an AI is waiting OR a clear root blocker exists) */}
        {waitingAlert ? (
          <div className="flex shrink-0 items-center gap-2 border-b border-[var(--accent-border)] bg-[var(--accent-soft)]/50 px-4 py-1.5 text-[12px]">
            <HelpCircle className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
            <span className="shrink-0 font-medium text-[var(--accent)]">
              {t.board.waitingBannerPrefix}
            </span>
            <span className="min-w-0 flex-1 truncate text-[var(--fg)]">{waitingAlert.reason}</span>
            <Button size="sm" variant="outline" onClick={() => setActiveView("mission")}>
              {t.board.goMissionBtn}
            </Button>
          </div>
        ) : recommendedBlocker ? (
          <div className="flex shrink-0 items-center gap-2 border-b border-[var(--border-subtle)] bg-[var(--surface-subtle)] px-4 py-1.5 text-[12px]">
            <Zap className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
            <span className="shrink-0 text-[var(--fg-2)]">{t.board.nextFocusPrefix}:</span>
            <button
              type="button"
              onClick={() => {
                setSplitDismissed(false);
                openDrawer(recommendedBlocker.issue.id);
              }}
              className="min-w-0 flex-1 truncate text-left font-medium text-[var(--fg)] hover:text-[var(--accent)] cursor-pointer"
            >
              <span className="mr-1.5 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                {recommendedBlocker.issue.shortId}
              </span>
              {recommendedBlocker.issue.title}
            </button>
            <span className="hidden shrink-0 whitespace-nowrap text-[11px] text-[var(--success)] sm:inline">
              {t.board.unlocksCount} {recommendedBlocker.hotspot.directUnblocks} {t.board.itemsUnit}
            </span>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void handleCopyPrompt(recommendedBlocker.issue)}
            >
              <Sparkles className="h-3 w-3 text-[var(--accent)]" />
              <span>{t.board.copyAiBtn}</span>
            </Button>
          </div>
        ) : null}

        {/* Always-ready Inline Quick-Add Strip */}
        <div className="flex shrink-0 items-center gap-2 border-b border-[var(--border-subtle)] bg-[var(--surface)]/60 px-4 py-1.5">
          <Plus className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
          <input
            type="text"
            value={quickTitle}
            disabled={quickBusy}
            onChange={(e) => setQuickTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleQuickCreate();
              } else if (e.key === "Escape") {
                setQuickTitle("");
                (e.target as HTMLInputElement).blur();
              }
            }}
            placeholder={t.board.quickAddPlaceholder}
            className="min-w-0 flex-1 bg-transparent text-[12.5px] text-[var(--fg)] outline-none placeholder:text-[var(--fg-3)]"
          />
          {quickTitle.trim() && (
            <Button
              size="sm"
              variant="primary"
              disabled={quickBusy}
              onClick={() => void handleQuickCreate()}
            >
              {t.actions.add}
            </Button>
          )}
        </div>

        {/* Scrollable Grouped Task Rows */}
        <div className="min-h-0 flex-1 overflow-y-auto">
          {groups.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 px-6 py-14 text-center">
              <div className="text-[13.5px] font-medium text-[var(--fg)]">
                {tasksFilter === "open" ? t.board.emptyOpenTitle : t.board.emptyFiltered}
              </div>
              <p className="max-w-[360px] text-[12px] text-[var(--fg-3)]">
                {tasksFilter === "open" ? t.board.emptyOpenHint : ""}
              </p>
              {tasksFilter === "open" && counts.closed > 0 && (
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => setTasksFilter("closed")}
                  className="mt-1"
                >
                  <span>
                    {t.board.viewClosedLink} ({counts.closed})
                  </span>
                  <ArrowRight className="h-3 w-3" />
                </Button>
              )}
            </div>
          ) : (
            <div className="pb-6">
              {groups.map((group) => {
                const isCollapsed = Boolean(collapsedGroups[group.key]);
                return (
                  <section key={group.key} className="border-b border-[var(--border-subtle)]">
                    <button
                      type="button"
                      onClick={() =>
                        setCollapsedGroups((prev) => ({
                          ...prev,
                          [group.key]: !prev[group.key],
                        }))
                      }
                      className="sticky top-0 z-10 flex w-full items-center gap-2 bg-[var(--bg)]/95 px-4 py-2 text-left backdrop-blur-xs hover:bg-[var(--surface-hover)]/60 cursor-pointer"
                    >
                      {isCollapsed ? (
                        <ChevronRight className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
                      ) : (
                        <ChevronDown className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
                      )}
                      {group.lane && <StatusRing lane={group.lane} size={12} />}
                      <span className="min-w-0 truncate text-[12px] font-semibold text-[var(--fg-2)]">
                        {group.label}
                      </span>
                      <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                        {group.items.length}
                      </span>
                      {group.progress && (
                        <span className="ml-auto shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                          {group.progress}
                        </span>
                      )}
                    </button>

                    {!isCollapsed && (
                      <div className="divide-y divide-[var(--border-subtle)]/70">
                        {group.items.map((it) => {
                          const isSelected = (selectedIssueId ?? splitIssueId) === it.id;
                          const isClosed = it.lane === "closed";
                          const openBlockers = it.openBlockedBy
                            .map((bid) => byId[bid])
                            .filter((x): x is IssueDto => Boolean(x));
                          const unlocksCount = it.blocks.filter(
                            (uid) => byId[uid]?.status !== "closed",
                          ).length;
                          const runtimeBadge = badgeByIssueId[it.id];

                          return (
                            <div
                              key={it.id}
                              ref={(el) => {
                                rowRefs.current[it.id] = el;
                              }}
                              className={`group relative flex items-center gap-2.5 border-l-2 px-3.5 py-2.5 transition-colors ${
                                isSelected
                                  ? "border-l-[var(--accent)] bg-[var(--surface-active)]/65"
                                  : "border-l-transparent hover:bg-[var(--surface-hover)]/65"
                              }`}
                            >
                              <button
                                type="button"
                                onClick={() => {
                                  setSplitDismissed(false);
                                  openDrawer(it.id);
                                }}
                                className="flex min-w-0 flex-1 items-center gap-2.5 text-left cursor-pointer"
                              >
                                <StatusRing lane={it.lane} />
                                <PriorityGlyph priority={it.priority} />
                                <span className="shrink-0 whitespace-nowrap font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                                  {it.shortId}
                                </span>
                                {it.issueType === "bug" && (
                                  <span className="shrink-0 whitespace-nowrap rounded-[var(--radius-xs)] bg-[var(--danger)]/10 px-1 py-0.25 text-[10.5px] text-[var(--danger)]">
                                    {t.types.bug}
                                  </span>
                                )}
                                <span
                                  className={`min-w-0 flex-1 truncate text-[13px] ${
                                    isClosed
                                      ? "font-normal text-[var(--fg-2)]"
                                      : "font-medium text-[var(--fg)]"
                                  }`}
                                >
                                  {it.title}
                                </span>
                              </button>

                              {/* Right metadata strip (fades cleanly on hover to reveal 1-click actions) */}
                              <div className="flex shrink-0 items-center gap-2 transition-opacity duration-100 group-hover:opacity-0">
                                {runtimeBadge && (
                                  <span
                                    className={`inline-flex items-center gap-1 whitespace-nowrap rounded-[var(--radius-xs)] px-1.5 py-0.5 text-[10.5px] font-medium ${
                                      runtimeBadge.kind === "waiting"
                                        ? "bg-[var(--accent-soft)] text-[var(--accent)]"
                                        : runtimeBadge.kind === "conflict"
                                          ? "bg-[var(--danger)]/10 text-[var(--danger)]"
                                          : runtimeBadge.kind === "working"
                                            ? "bg-[var(--success-soft)] text-[var(--success)]"
                                            : "bg-[var(--surface-subtle)] text-[var(--fg-3)]"
                                    }`}
                                  >
                                    {runtimeBadge.kind === "waiting"
                                      ? t.board.badgeWaiting
                                      : runtimeBadge.kind === "conflict"
                                        ? t.board.badgeConflict
                                        : runtimeBadge.label}
                                  </span>
                                )}

                                {openBlockers.length > 0 && openBlockers[0] ? (
                                  <span className="whitespace-nowrap text-[11px] font-medium text-[var(--danger)]">
                                    {t.board.blockedByPrefix} {openBlockers[0].shortId}
                                  </span>
                                ) : it.lane === "ready" && unlocksCount > 0 ? (
                                  <span className="whitespace-nowrap text-[11px] text-[var(--success)]">
                                    {t.board.unlocksCount} {unlocksCount}
                                  </span>
                                ) : null}

                                {!activeModule && it.modules.length > 0 && (
                                  <div className="hidden items-center gap-1.5 xl:flex">
                                    <ModuleInlineList
                                      modules={it.modules}
                                      prefix={modulePrefix}
                                      locale={locale}
                                      aliases={moduleAliases}
                                      limit={1}
                                    />
                                  </div>
                                )}

                                {it.assignee && (
                                  <div className="hidden sm:flex">
                                    <AssigneeDot name={it.assignee} />
                                  </div>
                                )}
                              </div>

                              {/* 1-Click Hover Actions */}
                              <div className="pointer-events-none absolute top-1/2 right-3 flex -translate-y-1/2 items-center gap-1 opacity-0 transition-opacity duration-100 group-hover:pointer-events-auto group-hover:opacity-100">
                                <button
                                  type="button"
                                  title={t.board.copyAiBtn}
                                  onClick={() => void handleCopyPrompt(it)}
                                  className="inline-flex h-6 items-center gap-1 whitespace-nowrap rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 text-[11px] text-[var(--fg-2)] shadow-[var(--shadow-xs)] hover:border-[var(--accent-border)] hover:bg-[var(--accent-soft)] hover:text-[var(--accent)] cursor-pointer"
                                >
                                  <Sparkles className="h-3 w-3 shrink-0" />
                                  <span>{t.board.copyAiBtn}</span>
                                </button>
                                {it.lane === "ready" && (
                                  <button
                                    type="button"
                                    onClick={() => void handleQuickStart(it)}
                                    className="inline-flex h-6 items-center gap-1 whitespace-nowrap rounded-[var(--radius-xs)] border border-[var(--accent-border)] bg-[var(--accent-soft)] px-2 text-[11px] font-medium text-[var(--accent)] shadow-[var(--shadow-xs)] hover:brightness-105 cursor-pointer"
                                  >
                                    <Play className="h-3 w-3 shrink-0" />
                                    <span>{t.board.startBtn}</span>
                                  </button>
                                )}
                                {it.lane === "in_progress" && (
                                  <button
                                    type="button"
                                    onClick={() => onRequestClose(it)}
                                    className="inline-flex h-6 items-center gap-1 whitespace-nowrap rounded-[var(--radius-xs)] border border-[var(--success)]/40 bg-[var(--success-soft)] px-2 text-[11px] font-medium text-[var(--success)] shadow-[var(--shadow-xs)] hover:brightness-105 cursor-pointer"
                                  >
                                    <Check className="h-3 w-3 shrink-0" />
                                    <span>{t.actions.done}</span>
                                  </button>
                                )}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </section>
                );
              })}

              {hiddenClosedRoundsCount > 0 && (
                <div className="px-4 pt-3">
                  <button
                    type="button"
                    onClick={() => setHideClosedRounds(false)}
                    className="w-full rounded-[var(--radius-sm)] border border-dashed border-[var(--border)] py-2 text-center text-[11.5px] text-[var(--fg-3)] hover:border-[var(--border-strong)] hover:text-[var(--fg)] cursor-pointer"
                  >
                    +{hiddenClosedRoundsCount} {t.board.hiddenRoundsHint} · {t.board.showRounds}
                  </button>
                </div>
              )}

              {tasksFilter === "open" && counts.closed > 0 && (
                <div className="flex items-center justify-between px-4 pt-3 text-[11.5px] text-[var(--fg-3)]">
                  <span>{t.board.keyboardNavHint}</span>
                  <button
                    type="button"
                    onClick={() => setTasksFilter("closed")}
                    className="inline-flex items-center gap-1 hover:text-[var(--fg)] cursor-pointer"
                  >
                    <span>
                      {t.board.viewClosedLink} ({counts.closed})
                    </span>
                    <ArrowRight className="h-3 w-3" />
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Right Permanent Split-Pane Preview on wide screens (zero modal backdrop fatigue!) */}
      {isWide && !splitDismissed && (
        <aside className="flex w-[390px] xl:w-[450px] shrink-0 flex-col border-l border-[var(--border)] bg-[var(--surface)]">
          {splitIssueId ? (
            <IssueDetailPanel
              issueId={splitIssueId}
              modulePrefix={modulePrefix}
              moduleAliases={moduleAliases}
              onRequestClose={onRequestClose}
              onClose={() => {
                setSplitDismissed(true);
                closeDrawer();
              }}
              mode="split"
            />
          ) : (
            <div className="grid flex-1 place-items-center p-6 text-center text-[12px] text-[var(--fg-3)]">
              {t.board.selectTaskHint}
            </div>
          )}
        </aside>
      )}
    </div>
  );
}
