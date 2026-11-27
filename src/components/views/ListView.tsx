import { useMemo, useState } from "react";
import { AssigneeDot, ModuleInlineList, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { BoardLane, IssueDto, ModuleAliasConfig } from "@/types/ipc";

export interface ListViewProps {
  issues: IssueDto[];
  epics: IssueDto[];
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
}

export function ListView({ issues, epics, modulePrefix, moduleAliases }: ListViewProps) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const byId = useWorkspaceStore((s) => s.byId);
  const { listLaneFilter, setListLaneFilter, openDrawer } = useUiStore();

  const [prioFilter, setPrioFilter] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [epicFilter, setEpicFilter] = useState("");
  const [assigneeFilter, setAssigneeFilter] = useState("");

  const assignees = useMemo(
    () => [...new Set(issues.map((i) => i.assignee).filter(Boolean))].sort(),
    [issues],
  );

  const filtered = useMemo(
    () =>
      issues.filter((it) => {
        if (listLaneFilter && it.lane !== listLaneFilter) return false;
        if (prioFilter !== "" && String(it.priority) !== prioFilter) return false;
        if (typeFilter && it.issueType !== typeFilter) return false;
        if (epicFilter && it.parentId !== epicFilter && it.id !== epicFilter) return false;
        if (assigneeFilter && it.assignee !== assigneeFilter) return false;
        return true;
      }),
    [issues, listLaneFilter, prioFilter, typeFilter, epicFilter, assigneeFilter],
  );

  const selClass =
    "h-7 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2 text-[12px] text-[var(--fg-2)] focus:border-[var(--border-strong)] focus:text-[var(--fg)] cursor-pointer";

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-[var(--border)] px-4 py-2">
        <select
          value={listLaneFilter}
          onChange={(e) => setListLaneFilter(e.target.value as BoardLane | "")}
          className={selClass}
        >
          <option value="">{t.list.allStatus}</option>
          <option value="in_progress">{t.lanes.in_progress}</option>
          <option value="ready">{t.lanes.ready}</option>
          <option value="blocked">{t.lanes.blocked}</option>
          <option value="closed">{t.lanes.closed}</option>
        </select>

        <select
          value={prioFilter}
          onChange={(e) => setPrioFilter(e.target.value)}
          className={selClass}
        >
          <option value="">{t.list.allPrio}</option>
          <option value="0">P0</option>
          <option value="1">P1</option>
          <option value="2">P2</option>
          <option value="3">P3</option>
        </select>

        <select
          value={typeFilter}
          onChange={(e) => setTypeFilter(e.target.value)}
          className={selClass}
        >
          <option value="">{t.list.allType}</option>
          <option value="epic">{t.types.epic}</option>
          <option value="task">{t.types.task}</option>
          <option value="bug">{t.types.bug}</option>
          <option value="feature">{t.types.feature}</option>
          <option value="chore">{t.types.chore}</option>
        </select>

        <select
          value={epicFilter}
          onChange={(e) => setEpicFilter(e.target.value)}
          className={selClass}
        >
          <option value="">{t.list.allEpic}</option>
          {epics.map((e) => (
            <option key={e.id} value={e.id}>
              {e.shortId} · {e.title.slice(0, 26)}
            </option>
          ))}
        </select>

        <select
          value={assigneeFilter}
          onChange={(e) => setAssigneeFilter(e.target.value)}
          className={selClass}
        >
          <option value="">{t.list.allAssignee}</option>
          {assignees.map((a) => (
            <option key={a} value={a}>
              {a}
            </option>
          ))}
        </select>

        <span className="ml-auto font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
          {filtered.length} / {issues.length}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full border-collapse">
          <thead>
            <tr className="border-b border-[var(--border)] bg-[var(--bg)] text-left text-[11px] font-medium whitespace-nowrap text-[var(--fg-3)]">
              <th className="sticky top-0 z-10 w-[68px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colPrio}
              </th>
              <th className="sticky top-0 z-10 w-[88px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colId}
              </th>
              <th className="sticky top-0 z-10 bg-[var(--bg)] px-3.5 py-2">{t.list.colTitle}</th>
              <th className="sticky top-0 z-10 w-[132px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colMod}
              </th>
              <th className="sticky top-0 z-10 w-[104px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colStatus}
              </th>
              <th className="sticky top-0 z-10 w-[124px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colAssignee}
              </th>
              <th className="sticky top-0 z-10 w-[132px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colRef}
              </th>
              <th className="sticky top-0 z-10 w-[124px] bg-[var(--bg)] px-3.5 py-2">
                {t.list.colBlocker}
              </th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((it) => {
              const refText = it.smart.commits[0] ?? it.smart.branches[0]?.split("/").pop() ?? "—";
              const openBlks = it.openBlockedBy.map((id) => byId[id]?.shortId ?? id);

              return (
                <tr
                  key={it.id}
                  onClick={() => openDrawer(it.id)}
                  className="border-b border-[var(--border-subtle)] transition-colors hover:bg-[var(--surface-hover)] cursor-pointer"
                >
                  <td className="whitespace-nowrap px-3.5 py-2">
                    <div className="flex items-center gap-1.5 font-mono-tabular text-[11.5px] text-[var(--fg-2)]">
                      <PriorityGlyph priority={it.priority} />
                      <span>P{it.priority}</span>
                    </div>
                  </td>
                  <td className="whitespace-nowrap px-3.5 py-2 font-mono-tabular text-[11.5px] text-[var(--fg-3)]">
                    {it.shortId}
                  </td>
                  <td className="px-3.5 py-2">
                    <div className="text-[12.5px] font-medium text-[var(--fg)]">{it.title}</div>
                    {it.parentTitle && (
                      <div className="mt-0.5 text-[11px] text-[var(--fg-3)]">
                        {it.parentShortId} · {it.parentTitle}
                      </div>
                    )}
                  </td>
                  <td className="px-3.5 py-2">
                    <div className="flex flex-wrap gap-2">
                      {it.modules.length > 0 ? (
                        <ModuleInlineList
                          modules={it.modules}
                          prefix={modulePrefix}
                          locale={locale}
                          aliases={moduleAliases}
                          limit={2}
                        />
                      ) : (
                        <span className="text-[var(--fg-3)]">—</span>
                      )}
                    </div>
                  </td>
                  <td className="px-3.5 py-2">
                    <span className="inline-flex items-center gap-1.5 whitespace-nowrap text-[12px] text-[var(--fg-2)]">
                      <StatusRing lane={it.lane} />
                      <span>{t.lanes[it.lane]}</span>
                    </span>
                  </td>
                  <td className="px-3.5 py-2">
                    {it.assignee ? (
                      <AssigneeDot name={it.assignee} />
                    ) : (
                      <span className="text-[var(--fg-3)]">—</span>
                    )}
                  </td>
                  <td className="px-3.5 py-2 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                    {refText}
                  </td>
                  <td className="px-3.5 py-2 text-[11.5px]">
                    {openBlks.length > 0 ? (
                      <span className="text-[var(--danger)]">
                        {t.board.blockedByPrefix} {openBlks.join(", ")}
                      </span>
                    ) : (
                      <span className="text-[var(--fg-3)]">—</span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
