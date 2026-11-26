import { ChevronDown, FolderOpen, Plus, X } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { Popover } from "@/components/ui/Popover";
import { useT } from "@/i18n";
import { formatIpcError, ipcPickFolder } from "@/lib/ipc";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";

export function WorkspaceSwitcher({ collapsed }: { collapsed: boolean }) {
  const t = useT();
  const {
    workspaces,
    activePath,
    snapshot,
    selectWorkspace,
    addWorkspace,
    removeWorkspace,
    initBeads,
  } = useWorkspaceStore();
  const { setActiveModule, closeDrawer, pushToast } = useUiStore();
  const [manualPath, setManualPath] = useState("");
  const [busyPath, setBusyPath] = useState<string | null>(null);

  const currentWs = workspaces.find(
    (w) => activePath && w.path.toLowerCase() === activePath.toLowerCase(),
  );
  const currentName = currentWs?.name ?? snapshot?.repoName ?? t.workspace.noneSelected;

  const activeRepos = workspaces.filter((w) => w.hasBeads);
  const otherRepos = workspaces.filter((w) => !w.hasBeads);

  const handleSelect = async (path: string) => {
    setActiveModule("");
    closeDrawer();
    try {
      await selectWorkspace(path);
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handlePickFolder = async () => {
    try {
      const picked = await ipcPickFolder(t.workspace.pickFolderTitle);
      if (!picked) return;
      await addWorkspace(picked);
      setActiveModule("");
      closeDrawer();
      pushToast(t.workspace.addedToast, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handleAddManual = async () => {
    const raw = manualPath.trim();
    if (!raw) return;
    try {
      await addWorkspace(raw);
      setManualPath("");
      setActiveModule("");
      closeDrawer();
      pushToast(t.workspace.addedToast, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handleInit = async (path: string) => {
    setBusyPath(path);
    try {
      await initBeads(path);
      pushToast(t.workspace.initSuccess, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setBusyPath(null);
    }
  };

  const handleRemove = async (path: string) => {
    try {
      await removeWorkspace(path);
      pushToast(t.workspace.removedToast, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  if (collapsed) {
    return (
      <div className="flex justify-center border-b border-[var(--border)] py-2">
        <Button
          size="icon"
          variant="ghost"
          aria-label={t.workspace.addFolder}
          title={currentName}
          onClick={() => void handlePickFolder()}
          className="h-7 w-7"
        >
          <FolderOpen className="h-3.5 w-3.5" />
        </Button>
      </div>
    );
  }

  return (
    <div className="border-b border-[var(--border)] p-2">
      <Popover
        align="start"
        className="w-[310px] p-0 overflow-hidden"
        trigger={
          <button
            type="button"
            className="flex h-8 w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 text-left transition-colors hover:bg-[var(--surface-hover)] cursor-pointer"
          >
            <span className="h-2 w-2 shrink-0 rounded-full bg-[var(--accent)]" />
            <span className="min-w-0 flex-1 truncate text-[12.5px] font-semibold text-[var(--fg)]">
              {currentName}
            </span>
            {currentWs?.prefix && (
              <span className="font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                {currentWs.prefix}
              </span>
            )}
            <ChevronDown className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
          </button>
        }
      >
        <div className="flex flex-col">
          <div className="flex items-center justify-between px-3 pt-2.5 pb-1.5">
            <span className="text-[11px] font-medium text-[var(--fg-3)]">
              {t.workspace.initializedSection}
            </span>
            <Button size="sm" variant="outline" onClick={() => void handlePickFolder()}>
              <FolderOpen className="h-3 w-3" />
              <span>{t.workspace.pickFolder}</span>
            </Button>
          </div>

          <div className="max-h-[220px] overflow-y-auto px-1.5 pb-1.5">
            {activeRepos.length === 0 ? (
              <div className="px-2 py-2.5 text-[11.5px] text-[var(--fg-3)]">
                {t.workspace.emptyInitialized}
              </div>
            ) : (
              activeRepos.map((ws) => {
                const isCur = activePath?.toLowerCase() === ws.path.toLowerCase();
                return (
                  <div
                    key={ws.path}
                    className={`group flex items-center gap-1.5 rounded-[var(--radius-sm)] px-2 py-1.5 transition-colors ${
                      isCur ? "bg-[var(--surface-active)]" : "hover:bg-[var(--surface-hover)]"
                    }`}
                  >
                    <button
                      type="button"
                      onClick={() => void handleSelect(ws.path)}
                      className="min-w-0 flex-1 text-left cursor-pointer"
                    >
                      <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--fg)]">
                        {isCur && (
                          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--accent)]" />
                        )}
                        <span className="truncate">{ws.name}</span>
                        {ws.prefix && (
                          <span className="font-mono-tabular text-[10px] text-[var(--fg-3)]">
                            {ws.prefix}
                          </span>
                        )}
                      </div>
                      <div
                        className="truncate font-mono-tabular text-[10px] text-[var(--fg-3)]"
                        title={ws.path}
                      >
                        {ws.path}
                      </div>
                    </button>
                    <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">
                      {ws.issueCount}
                    </span>
                    {ws.isRegistered && (
                      <button
                        type="button"
                        aria-label={t.actions.remove}
                        onClick={() => void handleRemove(ws.path)}
                        className="opacity-0 group-hover:opacity-100 rounded p-0.5 text-[var(--fg-3)] hover:text-[var(--danger)] cursor-pointer"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    )}
                  </div>
                );
              })
            )}
          </div>

          {otherRepos.length > 0 && (
            <div className="border-t border-[var(--border)]">
              <div className="px-3 pt-2 pb-1 text-[11px] font-medium text-[var(--fg-3)]">
                {t.workspace.uninitializedSection}
              </div>
              <div className="max-h-[140px] overflow-y-auto px-1.5 pb-1.5">
                {otherRepos.map((ws) => (
                  <div
                    key={ws.path}
                    className="flex items-center gap-2 rounded-[var(--radius-sm)] px-2 py-1.5 hover:bg-[var(--surface-hover)]"
                  >
                    <button
                      type="button"
                      onClick={() => void handleSelect(ws.path)}
                      className="min-w-0 flex-1 text-left cursor-pointer"
                    >
                      <div className="truncate text-[12px] font-medium text-[var(--fg-2)]">
                        {ws.name}
                      </div>
                      <div
                        className="truncate font-mono-tabular text-[10px] text-[var(--fg-3)]"
                        title={ws.path}
                      >
                        {ws.path}
                      </div>
                    </button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busyPath === ws.path}
                      onClick={() => void handleInit(ws.path)}
                    >
                      {busyPath === ws.path ? "..." : t.workspace.initBtn}
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="flex items-center gap-1.5 border-t border-[var(--border)] bg-[var(--surface-subtle)] p-2">
            <input
              type="text"
              value={manualPath}
              onChange={(e) => setManualPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleAddManual();
              }}
              placeholder={t.workspace.pathPlaceholder}
              className="h-7 min-w-0 flex-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 font-mono-tabular text-[11px] text-[var(--fg)] focus:border-[var(--border-strong)]"
            />
            <Button
              size="sm"
              variant="primary"
              disabled={!manualPath.trim()}
              onClick={() => void handleAddManual()}
              className="h-7"
            >
              <Plus className="h-3 w-3" />
              <span>{t.workspace.connectBtn}</span>
            </Button>
          </div>
        </div>
      </Popover>
    </div>
  );
}
