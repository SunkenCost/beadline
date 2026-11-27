import { ArrowUpRight, FolderGit2 } from "lucide-react";
import { AssigneeDot, PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";

export function AllActiveView() {
  const t = useT();
  const { allActive, selectWorkspace } = useWorkspaceStore();
  const { setActiveView, setActiveModule, openDrawer } = useUiStore();

  const handleJump = async (workspacePath: string, issueId: string) => {
    setActiveModule("");
    await selectWorkspace(workspacePath);
    setActiveView("tasks");
    openDrawer(issueId);
  };

  if (allActive.length === 0) {
    return (
      <div className="grid flex-1 place-items-center p-8 text-center">
        <div className="max-w-[380px]">
          <div className="text-[13.5px] font-medium text-[var(--fg)]">{t.allActive.empty}</div>
          <p className="mt-1.5 text-[12px] text-[var(--fg-3)]">{t.allActive.jumpHint}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <div className="mx-auto max-w-[960px]">
        <div className="mb-3 flex items-center justify-between text-[11.5px] text-[var(--fg-3)]">
          <span>
            {t.allActive.title} · {allActive.length}
          </span>
          <span>{t.allActive.jumpHint}</span>
        </div>

        <div className="flex flex-col divide-y divide-[var(--border-subtle)] rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)]">
          {allActive.map(({ workspaceName, workspacePath, issue }) => (
            <button
              key={`${workspacePath}:${issue.id}`}
              type="button"
              onClick={() => void handleJump(workspacePath, issue.id)}
              className="group flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-[var(--surface-hover)] cursor-pointer"
            >
              <StatusRing lane={issue.lane} />
              <PriorityGlyph priority={issue.priority} />
              <span className="inline-flex shrink-0 items-center gap-1 font-mono-tabular text-[11px] text-[var(--accent)]">
                <FolderGit2 className="h-3 w-3" />
                <span>{workspaceName}</span>
              </span>
              <span className="shrink-0 font-mono-tabular text-[11px] text-[var(--fg-3)]">
                {issue.shortId}
              </span>
              <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-[var(--fg)]">
                {issue.title}
              </span>
              <AssigneeDot name={issue.assignee} />
              <ArrowUpRight className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)] opacity-0 transition-opacity group-hover:opacity-100" />
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
