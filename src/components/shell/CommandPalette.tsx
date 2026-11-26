import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import {
  Activity,
  FolderOpen,
  GitFork,
  Kanban,
  Layers,
  List,
  Plus,
  Radar,
  RefreshCw,
  Search,
  Settings,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { PriorityGlyph, StatusRing } from "@/components/ui/Icons";
import { useT } from "@/i18n";
import { formatIpcError, ipcPickFolder } from "@/lib/ipc";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";

interface PaletteItem {
  id: string;
  group: string;
  label: string;
  meta?: string;
  onRun: () => void;
  icon?: "issue" | "workspace" | "action";
  lane?: "in_progress" | "ready" | "blocked" | "closed";
  priority?: number;
}

export function CommandPalette() {
  const t = useT();
  const open = useUiStore((s) => s.paletteOpen);
  const setOpen = useUiStore((s) => s.setPaletteOpen);
  const {
    setActiveView,
    openDrawer,
    openCreateModal,
    setSettingsOpen,
    setActiveModule,
    pushToast,
  } = useUiStore();
  const { workspaces, snapshot, selectWorkspace, addWorkspace, syncGithub } = useWorkspaceStore();

  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setCursor(0);
      window.setTimeout(() => inputRef.current?.focus(), 20);
    }
  }, [open]);

  const items = useMemo<PaletteItem[]>(() => {
    const q = query.trim().toLowerCase();
    const list: PaletteItem[] = [];

    // Actions
    const actions: PaletteItem[] = [
      {
        id: "act-new",
        group: t.palette.groupActions,
        label: t.actions.newIssue,
        meta: "C",
        icon: "action",
        onRun: () => openCreateModal(),
      },
      {
        id: "act-add-ws",
        group: t.palette.groupActions,
        label: t.workspace.addFolder,
        icon: "action",
        onRun: () => {
          void ipcPickFolder(t.workspace.pickFolderTitle).then((p) => {
            if (p) {
              void addWorkspace(p)
                .then(() => pushToast(t.workspace.addedToast, "ok"))
                .catch((e) => pushToast(formatIpcError(e), "err"));
            }
          });
        },
      },
      {
        id: "act-sync-gh",
        group: t.palette.groupActions,
        label: t.actions.syncGithub,
        icon: "action",
        onRun: () => {
          void syncGithub(true)
            .then((out) => pushToast(out.split("\n")[0] || t.sync.ghDone, "ok"))
            .catch((e) => pushToast(formatIpcError(e), "err"));
        },
      },
      {
        id: "act-settings",
        group: t.palette.groupActions,
        label: t.nav.settings,
        meta: "Ctrl+,",
        icon: "action",
        onRun: () => setSettingsOpen(true),
      },
      {
        id: "v-mission",
        group: t.palette.groupViews,
        label: t.nav.mission,
        meta: "M",
        onRun: () => setActiveView("mission"),
      },
      {
        id: "v-board",
        group: t.palette.groupViews,
        label: t.nav.board,
        meta: "1",
        onRun: () => setActiveView("board"),
      },
      {
        id: "v-epics",
        group: t.palette.groupViews,
        label: t.nav.epics,
        meta: "2",
        onRun: () => setActiveView("epics"),
      },
      {
        id: "v-dag",
        group: t.palette.groupViews,
        label: t.nav.dag,
        meta: "3",
        onRun: () => setActiveView("dag"),
      },
      {
        id: "v-list",
        group: t.palette.groupViews,
        label: t.nav.list,
        meta: "4",
        onRun: () => setActiveView("list"),
      },
      {
        id: "v-all",
        group: t.palette.groupViews,
        label: t.nav.allActive,
        meta: "0",
        onRun: () => setActiveView("all-active"),
      },
    ];

    for (const ws of workspaces) {
      list.push({
        id: `ws-${ws.path}`,
        group: t.palette.groupWorkspaces,
        label: ws.name,
        meta: ws.path,
        icon: "workspace",
        onRun: () => {
          setActiveModule("");
          void selectWorkspace(ws.path);
        },
      });
    }

    for (const it of snapshot?.issues ?? []) {
      const hay =
        `${it.id} ${it.shortId} ${it.title} ${it.assignee} ${it.smart.githubNumber ?? ""}`.toLowerCase();
      if (!q || hay.includes(q)) {
        list.push({
          id: `issue-${it.id}`,
          group: t.palette.groupIssues,
          label: it.title,
          meta: it.shortId,
          icon: "issue",
          lane: it.lane,
          priority: it.priority,
          onRun: () => openDrawer(it.id),
        });
      }
    }

    const filteredActions = actions.filter(
      (a) => !q || `${a.label} ${a.group}`.toLowerCase().includes(q),
    );
    const filteredWs = list.filter(
      (x) =>
        x.icon === "workspace" && (!q || `${x.label} ${x.meta ?? ""}`.toLowerCase().includes(q)),
    );
    const filteredIssues = list.filter((x) => x.icon === "issue").slice(0, 14);

    return q
      ? [...filteredIssues, ...filteredWs, ...filteredActions]
      : [...filteredActions.slice(0, 6), ...filteredWs.slice(0, 5), ...filteredIssues.slice(0, 8)];
  }, [
    query,
    workspaces,
    snapshot,
    t,
    openCreateModal,
    addWorkspace,
    pushToast,
    syncGithub,
    setSettingsOpen,
    setActiveView,
    setActiveModule,
    selectWorkspace,
    openDrawer,
  ]);

  const activeIdx = Math.min(cursor, Math.max(0, items.length - 1));

  const runItem = (item: PaletteItem | undefined) => {
    if (!item) return;
    setOpen(false);
    item.onRun();
  };

  return (
    <BaseDialog.Root open={open} onOpenChange={setOpen}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className="fixed inset-0 z-50 bg-black/40" />
        <BaseDialog.Popup className="fixed top-[16%] left-1/2 z-50 flex max-h-[68vh] w-[min(580px,calc(100vw-32px))] -translate-x-1/2 flex-col overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border-strong)] bg-[var(--surface)] text-[var(--fg)] shadow-[var(--shadow-lg)]">
          <div className="flex items-center gap-2.5 border-b border-[var(--border)] px-3.5 py-2.5">
            <Search className="h-4 w-4 shrink-0 text-[var(--fg-3)]" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setCursor(0);
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setCursor((c) => Math.min(items.length - 1, c + 1));
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setCursor((c) => Math.max(0, c - 1));
                } else if (e.key === "Enter") {
                  e.preventDefault();
                  runItem(items[activeIdx]);
                }
              }}
              placeholder={t.palette.placeholder}
              className="w-full bg-transparent text-[13px] text-[var(--fg)] outline-none placeholder:text-[var(--fg-3)]"
            />
            <span className="font-mono-tabular text-[10.5px] text-[var(--fg-3)]">ESC</span>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto p-1.5">
            {items.length === 0 ? (
              <div className="px-3 py-8 text-center text-[12px] text-[var(--fg-3)]">
                {t.palette.noResults}
              </div>
            ) : (
              items.map((item, idx) => {
                const selected = idx === activeIdx;
                return (
                  <button
                    key={item.id}
                    type="button"
                    onMouseEnter={() => setCursor(idx)}
                    onClick={() => runItem(item)}
                    className={`flex w-full items-center gap-2.5 rounded-[var(--radius-sm)] px-2.5 py-2 text-left text-[12.5px] cursor-pointer ${
                      selected
                        ? "bg-[var(--surface-active)] text-[var(--fg)]"
                        : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)]"
                    }`}
                  >
                    {item.icon === "issue" && item.lane && (
                      <span className="inline-flex items-center gap-1.5">
                        <StatusRing lane={item.lane} />
                        {item.priority !== undefined && <PriorityGlyph priority={item.priority} />}
                      </span>
                    )}
                    {item.icon === "workspace" && (
                      <FolderOpen className="h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
                    )}
                    {item.id === "act-new" && <Plus className="h-3.5 w-3.5 text-[var(--accent)]" />}
                    {item.id === "act-add-ws" && (
                      <FolderOpen className="h-3.5 w-3.5 text-[var(--fg-3)]" />
                    )}
                    {item.id === "act-sync-gh" && (
                      <RefreshCw className="h-3.5 w-3.5 text-[var(--fg-3)]" />
                    )}
                    {item.id === "act-settings" && (
                      <Settings className="h-3.5 w-3.5 text-[var(--fg-3)]" />
                    )}
                    {item.id === "v-mission" && (
                      <Radar className="h-3.5 w-3.5 text-[var(--accent)]" />
                    )}
                    {item.id === "v-board" && <Kanban className="h-3.5 w-3.5 text-[var(--fg-3)]" />}
                    {item.id === "v-epics" && <Layers className="h-3.5 w-3.5 text-[var(--fg-3)]" />}
                    {item.id === "v-dag" && <GitFork className="h-3.5 w-3.5 text-[var(--fg-3)]" />}
                    {item.id === "v-list" && <List className="h-3.5 w-3.5 text-[var(--fg-3)]" />}
                    {item.id === "v-all" && <Activity className="h-3.5 w-3.5 text-[var(--fg-3)]" />}

                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    {item.meta && (
                      <span className="max-w-[180px] truncate font-mono-tabular text-[11px] text-[var(--fg-3)]">
                        {item.meta}
                      </span>
                    )}
                  </button>
                );
              })
            )}
          </div>
        </BaseDialog.Popup>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}
