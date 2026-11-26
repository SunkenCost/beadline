import { Check, ChevronDown, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";
import { AssigneeDot, ModuleInlineList, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { extractPlainSummary, isClosedIterationRound, shortParentLabel } from "@/lib/human-context";
import { formatIpcError } from "@/lib/ipc";
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
  const { byId, updateIssue } = useWorkspaceStore();
  const { boardGroupMode, hideClosedRounds, setHideClosedRounds, openDrawer, pushToast } =
    useUiStore();
  const [dragId, setDragId] = useState<string | null>(null);
  const [overTarget, setOverTarget] = useState<string | null>(null);
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({});

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

  const renderCard = (it: IssueDto, isClosed: boolean, showParent: boolean) => {
    const openBlockers = it.openBlockedBy
      .map((bid) => byId[bid])
      .filter((x): x is IssueDto => Boolean(x));
    const unlocksCount = it.blocks.filter((uid) => byId[uid]?.status !== "closed").length;
    const firstCommit = it.smart.commits[0];
    const hasQuickDone = it.lane === "in_progress";
    const parentReadable =
      showParent && it.parentId && it.parentTitle && it.issueType !== "epic"
        ? shortParentLabel(it.parentTitle, 18)
        : null;
    const showFoot =
      it.modules.length > 0 ||
      openBlockers.length > 0 ||
      (it.lane === "ready" && unlocksCount > 0) ||
      Boolean(firstCommit) ||
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
        className={`group relative rounded-[var(--radius-md)] border px-[var(--card-pad-x)] py-[var(--card-pad-y)] transition-[border-color,background-color,box-shadow] duration-100 cursor-pointer ${
          dragId === it.id ? "opacity-40" : ""
        } ${
          isClosed
            ? "border-[var(--border-subtle)] bg-transparent hover:border-[var(--border-strong)] hover:bg-[var(--surface)]"
            : "border-[var(--border)] bg-[var(--surface)] hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-sm)]"
        }`}
      >
        {/* Row 1: Priority glyph + mono ID + optional Epic/Bug + #N + right slot */}
        <div className="mb-1.5 flex min-h-[18px] items-center gap-1.5">
          <PriorityGlyph priority={it.priority} />
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              void navigator.clipboard?.writeText(it.id);
              pushToast(`${t.drawer.copiedId}: ${it.id}`, "ok");
            }}
            title={`${t.actions.copyId}: ${it.id}`}
            className="font-mono-tabular text-[11px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
          >
            {it.shortId}
          </button>
          {it.issueType === "epic" && (
            <span className="text-[11px] text-[var(--fg-3)]">{t.types.epic}</span>
          )}
          {it.issueType === "bug" && (
            <span className="text-[11px] text-[var(--danger)]">{t.types.bug}</span>
          )}
          {it.smart.githubNumber && (
            <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">
              #{it.smart.githubNumber}
            </span>
          )}

          <div className="relative ml-auto flex min-h-[20px] items-center justify-end">
            <div
              className={`transition-opacity duration-100 ${hasQuickDone ? "group-hover:opacity-0" : ""}`}
            >
              <AssigneeDot name={it.assignee} />
            </div>
            {hasQuickDone && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onRequestClose(it);
                }}
                className="pointer-events-none absolute top-1/2 right-0 inline-flex h-5 -translate-y-1/2 items-center gap-0.5 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-hover)] px-1.5 text-[11px] text-[var(--fg-2)] opacity-0 transition-all duration-100 group-hover:pointer-events-auto group-hover:opacity-100 hover:border-[var(--success)] hover:bg-[var(--success-soft)] hover:text-[var(--success)] cursor-pointer"
              >
                <Check className="h-3 w-3" />
                <span>{t.actions.done}</span>
              </button>
            )}
          </div>
        </div>

        {/* Row 2: Clean 2-line title */}
        <div
          className={`line-clamp-2 text-[13px] leading-[1.45] ${
            isClosed
              ? "font-normal text-[var(--fg-2)] group-hover:text-[var(--fg)]"
              : "font-medium text-[var(--fg)]"
          }`}
        >
          {it.title}
        </div>

        {/* Row 3: Borderless inline metadata with human-readable parent epic name */}
        {showFoot && (
          <div className="mt-2 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] text-[var(--fg-3)]">
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
                className="text-[11px] text-[var(--danger)] hover:underline underline-offset-2 cursor-pointer"
              >
                {t.board.blockedByPrefix} {openBlockers[0].shortId}
                {openBlockers.length > 1 ? ` +${openBlockers.length - 1}` : ""}
              </button>
            ) : it.lane === "ready" && unlocksCount > 0 ? (
              <span className="text-[11px] text-[var(--success)]">
                {t.board.unlocksCount} {unlocksCount} {t.board.itemsUnit}
              </span>
            ) : firstCommit ? (
              <span className="font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                {firstCommit}
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
                className="max-w-[165px] truncate text-[11px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
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
                <div className="grid grid-cols-4 overflow-x-auto border-t border-[var(--border-subtle)]">
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
                        className={`flex min-w-[230px] flex-col gap-[var(--lane-gap)] border-r border-[var(--border-subtle)] p-2.5 last:border-r-0 ${
                          overTarget === dropKey ? "bg-[var(--accent-soft)]" : ""
                        }`}
                      >
                        <div className="flex items-center gap-1.5 px-1 pb-0.5 text-[11px] text-[var(--fg-3)]">
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

  // Flat 4-column hairline Kanban
  return (
    <div className="grid min-h-0 flex-1 grid-cols-4 overflow-x-auto overflow-y-hidden">
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
            className={`flex min-h-0 min-w-[248px] flex-col border-r border-[var(--border-subtle)] last:border-r-0 transition-colors ${
              isDragOver ? "bg-[var(--accent-soft)]" : ""
            }`}
          >
            {/* Flat hairline column header — zero boxes, zero redundant subtitles */}
            <header className="flex shrink-0 items-center gap-2 px-3.5 pt-3 pb-2">
              <StatusRing lane={lane} />
              <h2 className="text-[12.5px] font-semibold text-[var(--fg)]">{t.lanes[lane]}</h2>
              <span className="font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                {allCards.length}
              </span>
            </header>

            <div className="flex min-h-0 flex-1 flex-col gap-[var(--lane-gap)] overflow-y-auto px-2.5 pt-1 pb-4">
              {cards.length === 0 && hiddenRounds === 0 ? (
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
            </div>
          </div>
        );
      })}
    </div>
  );
}
