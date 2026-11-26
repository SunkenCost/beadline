import { ExternalLink } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { AssigneeDot, ModuleInlineList, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { ipcOpenExternalLink } from "@/lib/ipc";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { IssueDto, ModuleAliasConfig } from "@/types/ipc";

export interface EpicsViewProps {
  epics: IssueDto[];
  activeModule: string;
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
}

export function EpicsView({ epics, activeModule, modulePrefix, moduleAliases }: EpicsViewProps) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const byId = useWorkspaceStore((s) => s.byId);
  const { openDrawer, openCreateModal } = useUiStore();

  const active = epics.filter((e) => e.status !== "closed");
  const closed = epics.filter((e) => e.status === "closed");

  const renderCard = (ep: IssueDto) => {
    const allKids = ep.childrenIds.map((id) => byId[id]).filter((x): x is IssueDto => Boolean(x));
    const kids = activeModule
      ? allKids.filter((k) => k.modules.includes(activeModule) || ep.modules.includes(activeModule))
      : allKids;
    const total = allKids.length;
    const done = allKids.filter((k) => k.status === "closed").length;
    const pct = total ? Math.round((done / total) * 100) : ep.status === "closed" ? 100 : 0;

    return (
      <article
        key={ep.id}
        className="flex flex-col gap-3 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] p-4 transition-colors hover:border-[var(--border-strong)]"
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <div className="mb-1 flex flex-wrap items-center gap-2 text-[11.5px] text-[var(--fg-3)]">
              <PriorityGlyph priority={ep.priority} />
              <span className="font-mono-tabular">{ep.shortId}</span>
              <ModuleInlineList
                modules={ep.modules}
                prefix={modulePrefix}
                locale={locale}
                aliases={moduleAliases}
                limit={3}
              />
              {ep.smart.githubNumber && ep.smart.githubUrl && (
                <button
                  type="button"
                  onClick={() => void ipcOpenExternalLink(ep.smart.githubUrl ?? "")}
                  className="inline-flex items-center gap-1 font-mono-tabular text-[11px] text-[var(--fg-2)] hover:text-[var(--accent)] cursor-pointer"
                >
                  <span>GitHub #{ep.smart.githubNumber}</span>
                  <ExternalLink className="h-3 w-3" />
                </button>
              )}
            </div>
            <button
              type="button"
              onClick={() => openDrawer(ep.id)}
              className="text-left text-[13.5px] font-semibold leading-snug text-[var(--fg)] hover:text-[var(--accent)] cursor-pointer"
            >
              {ep.title}
            </button>
          </div>
          <span
            className={`shrink-0 font-mono-tabular text-[14px] font-semibold ${
              pct === 100 ? "text-[var(--success)]" : "text-[var(--fg-2)]"
            }`}
          >
            {pct}%
          </span>
        </div>

        <div className="h-[3px] overflow-hidden rounded-full bg-[var(--border)]">
          <div
            className={`h-full rounded-full transition-[width] duration-300 ${
              pct === 100 ? "bg-[var(--success)]" : "bg-[var(--accent)]"
            }`}
            style={{ width: `${pct}%` }}
          />
        </div>

        {kids.length > 0 ? (
          <div className="flex flex-col border-t border-[var(--border-subtle)] pt-1.5">
            {kids.map((k) => (
              <button
                key={k.id}
                type="button"
                onClick={() => openDrawer(k.id)}
                className="flex items-center gap-2 rounded-[var(--radius-xs)] px-1.5 py-1.5 text-left transition-colors hover:bg-[var(--surface-hover)] cursor-pointer"
              >
                <StatusRing lane={k.lane} />
                <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                  {k.shortId}
                </span>
                <span
                  className={`min-w-0 flex-1 truncate text-[12.5px] ${
                    k.status === "closed" ? "text-[var(--fg-3)] line-through" : "text-[var(--fg)]"
                  }`}
                >
                  {k.title}
                </span>
                <AssigneeDot name={k.assignee} />
              </button>
            ))}
          </div>
        ) : (
          <div className="text-[12px] text-[var(--fg-3)]">{ep.notes || t.epics.noChildren}</div>
        )}

        <div className="flex items-center justify-between border-t border-[var(--border-subtle)] pt-2 text-[11.5px] text-[var(--fg-3)]">
          <span className="font-mono-tabular">
            {total > 0
              ? `${done} / ${total} ${t.epics.completedUnit}`
              : ep.openBlockedBy.length > 0
                ? `${t.board.blockedByPrefix} ${ep.openBlockedBy.map((id) => byId[id]?.shortId ?? id).join(", ")}`
                : t.epics.standalone}
          </span>
          <div className="flex items-center gap-1">
            <Button size="sm" variant="ghost" onClick={() => openCreateModal(ep.id)}>
              {t.actions.addSubtask}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => openDrawer(ep.id)}>
              {t.actions.details}
            </Button>
          </div>
        </div>
      </article>
    );
  };

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <div className="mx-auto max-w-[1320px]">
        <div className="mb-2.5 text-[11.5px] font-medium text-[var(--fg-3)]">
          {t.epics.activeSection} · {active.length}
        </div>
        {active.length === 0 ? (
          <div className="mb-6 text-[12.5px] text-[var(--fg-3)]">{t.epics.emptyActive}</div>
        ) : (
          <div className="mb-7 grid grid-cols-1 gap-3 xl:grid-cols-2">{active.map(renderCard)}</div>
        )}

        {closed.length > 0 && (
          <>
            <div className="mb-2.5 text-[11.5px] font-medium text-[var(--fg-3)]">
              {t.epics.closedSection} · {closed.length}
            </div>
            <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">{closed.map(renderCard)}</div>
          </>
        )}
      </div>
    </div>
  );
}
