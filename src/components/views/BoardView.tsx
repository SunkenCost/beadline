import { Check } from "lucide-react";
import { useState } from "react";
import { AssigneeDot, ModuleInlineList, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
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
  const { openDrawer, pushToast } = useUiStore();
  const [dragId, setDragId] = useState<string | null>(null);
  const [overLane, setOverLane] = useState<BoardLane | null>(null);

  const handleDrop = async (lane: BoardLane) => {
    setOverLane(null);
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

  return (
    <div className="grid min-h-0 flex-1 grid-cols-4 overflow-x-auto overflow-y-hidden">
      {LANES.map((lane) => {
        const cards = issues.filter((it) => it.lane === lane);
        const isClosed = lane === "closed";
        const isDragOver = overLane === lane;

        return (
          <div
            key={lane}
            role="listbox"
            aria-label={t.lanes[lane]}
            data-lane={lane}
            onDragOver={(e) => {
              e.preventDefault();
              if (overLane !== lane) setOverLane(lane);
            }}
            onDragLeave={() => {
              if (overLane === lane) setOverLane(null);
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
                {cards.length}
              </span>
            </header>

            <div className="flex min-h-0 flex-1 flex-col gap-[var(--lane-gap)] overflow-y-auto px-2.5 pt-1 pb-4">
              {cards.length === 0 ? (
                <div className="py-6 text-center text-[12px] text-[var(--fg-3)]">
                  {t.laneEmpty[lane]}
                </div>
              ) : (
                cards.map((it) => {
                  const openBlockers = it.openBlockedBy
                    .map((bid) => byId[bid])
                    .filter((x): x is IssueDto => Boolean(x));
                  const unlocksCount = it.blocks.filter(
                    (uid) => byId[uid]?.status !== "closed",
                  ).length;
                  const firstCommit = it.smart.commits[0];
                  const hasQuickDone = it.lane === "in_progress";
                  const showFoot =
                    it.modules.length > 0 ||
                    openBlockers.length > 0 ||
                    (it.lane === "ready" && unlocksCount > 0) ||
                    Boolean(firstCommit) ||
                    (!openBlockers.length && Boolean(it.parentShortId) && it.issueType !== "epic");

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
                        setOverLane(null);
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
                            className={`transition-opacity duration-100 ${
                              hasQuickDone ? "group-hover:opacity-0" : ""
                            }`}
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

                      {/* Row 3: Borderless inline metadata */}
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

                          {openBlockers.length === 0 &&
                            it.parentShortId &&
                            it.issueType !== "epic" && (
                              <span
                                className="max-w-[120px] truncate text-[11px] text-[var(--fg-3)]"
                                title={it.parentTitle ?? undefined}
                              >
                                {t.board.belongsTo} {it.parentShortId}
                              </span>
                            )}
                        </div>
                      )}
                    </article>
                  );
                })
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
