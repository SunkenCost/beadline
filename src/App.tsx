import {
  Activity,
  FolderOpen,
  GitFork,
  Kanban,
  Layers,
  List,
  Plus,
  RefreshCw,
  Search,
  Settings,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { IssueDrawer } from "@/components/drawer/IssueDrawer";
import { CloseIssueDialog } from "@/components/modals/CloseIssueDialog";
import { CreateIssueModal } from "@/components/modals/CreateIssueModal";
import { SettingsModal } from "@/components/settings/SettingsModal";
import { CommandPalette } from "@/components/shell/CommandPalette";
import { Titlebar } from "@/components/shell/Titlebar";
import { WorkspaceSwitcher } from "@/components/shell/WorkspaceSwitcher";
import { Button } from "@/components/ui/Button";
import { ToastStack } from "@/components/ui/Toast";
import { Tooltip, TooltipProvider } from "@/components/ui/Tooltip";
import { AllActiveView } from "@/components/views/AllActiveView";
import { BoardView } from "@/components/views/BoardView";
import { DagView } from "@/components/views/DagView";
import { EpicsView } from "@/components/views/EpicsView";
import { ListView } from "@/components/views/ListView";
import { useGlobalShortcuts } from "@/hooks/useGlobalShortcuts";
import { useSnapshotEvents } from "@/hooks/useSnapshotEvents";
import { useT } from "@/i18n";
import { formatIpcError, ipcPickFolder } from "@/lib/ipc";
import { resolveModule } from "@/lib/modules";
import { applyThemeToDocument } from "@/lib/theme";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { type ActiveView, useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { BoardLane, IssueDto } from "@/types/ipc";

const NAV_ITEMS: readonly {
  id: ActiveView;
  icon: typeof Kanban;
  labelKey: "board" | "epics" | "dag" | "list";
}[] = [
  { id: "board", icon: Kanban, labelKey: "board" },
  { id: "epics", icon: Layers, labelKey: "epics" },
  { id: "dag", icon: GitFork, labelKey: "dag" },
  { id: "list", icon: List, labelKey: "list" },
];

export function App() {
  const t = useT();
  const hydrateSettings = useSettingsStore((s) => s.hydrate);
  const theme = useSettingsStore((s) => s.settings.theme);
  const locale = useSettingsStore((s) => s.settings.language);
  const platform = useSettingsStore((s) => s.platform);

  const {
    workspaces,
    activePath,
    snapshot,
    allActive,
    syncStatus,
    loadAll,
    addWorkspace,
    initBeads,
    refreshSnapshot,
    syncGithub,
  } = useWorkspaceStore();

  const {
    activeView,
    sidebarCollapsed,
    activeModule,
    boardScope,
    search,
    setActiveView,
    setSettingsOpen,
    openCreateModal,
    openDrawer,
    setActiveModule,
    setBoardScope,
    setSearch,
    setListLaneFilter,
    pushToast,
  } = useUiStore();

  const [closingIssue, setClosingIssue] = useState<IssueDto | null>(null);
  const [welcomePath, setWelcomePath] = useState("");
  const [syncingGh, setSyncingGh] = useState(false);
  const [initBusy, setInitBusy] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const focusSearch = useCallback(() => {
    searchInputRef.current?.focus();
  }, []);

  useSnapshotEvents();
  useGlobalShortcuts(focusSearch);

  useEffect(() => {
    void hydrateSettings().then(() => loadAll());
  }, [hydrateSettings, loadAll]);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () => applyThemeToDocument(theme, mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, [theme]);

  const currentWs = useMemo(
    () =>
      workspaces.find((w) => activePath && w.path.toLowerCase() === activePath.toLowerCase()) ??
      null,
    [workspaces, activePath],
  );
  const modulePrefix = currentWs?.modulePrefix ?? snapshot?.modulePrefix ?? "mod:";
  const moduleAliases = currentWs?.moduleAliases;

  // Reset invalid active module on workspace switch
  useEffect(() => {
    if (activeModule && snapshot && !snapshot.modules.some((m) => m.key === activeModule)) {
      setActiveModule("");
    }
  }, [activeModule, snapshot, setActiveModule]);

  const matchesSearch = useCallback(
    (it: IssueDto) => {
      const q = search.trim().toLowerCase();
      if (!q) return true;
      const modLabels = it.modules.map(
        (k) => resolveModule(k, modulePrefix, locale, moduleAliases).label,
      );
      const hay = [
        it.id,
        it.shortId,
        it.title,
        it.description,
        it.notes,
        it.assignee,
        it.parentTitle ?? "",
        ...it.labels,
        ...modLabels,
        ...it.smart.branches,
        ...it.smart.commits,
        it.smart.githubNumber ? `#${it.smart.githubNumber}` : "",
      ]
        .join(" ")
        .toLowerCase();
      return hay.includes(q);
    },
    [search, modulePrefix, locale, moduleAliases],
  );

  const moduleScopedIssues = useMemo(() => {
    const all = snapshot?.issues ?? [];
    if (!activeModule) return all;
    return all.filter((it) => it.modules.includes(activeModule));
  }, [snapshot?.issues, activeModule]);

  const boardIssues = useMemo(
    () =>
      moduleScopedIssues.filter((it) => {
        if (boardScope === "tasks" && it.issueType === "epic") return false;
        return matchesSearch(it);
      }),
    [moduleScopedIssues, boardScope, matchesSearch],
  );

  const filteredEpics = useMemo(() => {
    const epics = snapshot?.epics ?? [];
    const byId = useWorkspaceStore.getState().byId;
    return epics.filter((e) => {
      const kids = e.childrenIds.map((id) => byId[id]).filter((x): x is IssueDto => Boolean(x));
      const modOk =
        !activeModule ||
        e.modules.includes(activeModule) ||
        kids.some((k) => k.modules.includes(activeModule));
      if (!modOk) return false;
      return matchesSearch(e) || kids.some(matchesSearch);
    });
  }, [snapshot?.epics, activeModule, matchesSearch]);

  const searchScopedIssues = useMemo(
    () => moduleScopedIssues.filter(matchesSearch),
    [moduleScopedIssues, matchesSearch],
  );

  const kpiCounts = useMemo(() => {
    const base =
      activeView === "board" && boardScope === "tasks"
        ? moduleScopedIssues.filter((i) => i.issueType !== "epic")
        : moduleScopedIssues;
    const c = { in_progress: 0, ready: 0, blocked: 0, closed: 0 };
    for (const it of base) c[it.lane] += 1;
    return c;
  }, [moduleScopedIssues, activeView, boardScope]);

  const sidebarLiveTasks = useMemo(
    () => moduleScopedIssues.filter((i) => i.lane === "in_progress" && i.issueType !== "epic"),
    [moduleScopedIssues],
  );

  const navCounts = useMemo(() => {
    const ids = new Set(moduleScopedIssues.map((i) => i.id));
    const edgeCount = (snapshot?.edges ?? []).filter(
      (e) => ids.has(e.from) && ids.has(e.to),
    ).length;
    return {
      board: moduleScopedIssues.filter((i) => i.issueType !== "epic").length,
      epics: moduleScopedIssues.filter((i) => i.issueType === "epic").length,
      dag: edgeCount,
      list: moduleScopedIssues.length,
    };
  }, [moduleScopedIssues, snapshot?.edges]);

  const activeModMeta = activeModule
    ? resolveModule(activeModule, modulePrefix, locale, moduleAliases)
    : null;

  const handleWelcomeBrowse = async () => {
    try {
      const picked = await ipcPickFolder(t.workspace.pickFolderTitle);
      if (!picked) return;
      await addWorkspace(picked);
      pushToast(t.workspace.addedToast, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handleWelcomeAdd = async () => {
    const p = welcomePath.trim();
    if (!p) return;
    try {
      await addWorkspace(p);
      setWelcomePath("");
      pushToast(t.workspace.addedToast, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    }
  };

  const handleInitCurrent = async () => {
    if (!activePath) return;
    setInitBusy(true);
    try {
      await initBeads(activePath);
      pushToast(t.workspace.initSuccess, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setInitBusy(false);
    }
  };

  const handleSyncGithub = async () => {
    setSyncingGh(true);
    try {
      const out = await syncGithub(true);
      pushToast(out.split("\n")[0] || t.sync.ghDone, "ok");
    } catch (err) {
      pushToast(formatIpcError(err), "err");
    } finally {
      setSyncingGh(false);
    }
  };

  const jumpToLaneList = (lane: BoardLane) => {
    setListLaneFilter(lane);
    setActiveView("list");
  };

  const hasReadyWorkspace = Boolean(snapshot?.hasBeads);
  const viewTitle =
    activeView === "all-active"
      ? t.nav.allActive
      : t.nav[NAV_ITEMS.find((n) => n.id === activeView)?.labelKey ?? "board"];

  return (
    <TooltipProvider delay={300}>
      <div className="flex h-screen w-screen flex-col overflow-hidden bg-[var(--bg)] text-[var(--fg)]">
        <Titlebar />

        <div className="flex min-h-0 flex-1 overflow-hidden">
          {/* Collapsible Minimal Sidebar */}
          <aside
            className={`flex shrink-0 flex-col border-r border-[var(--border)] bg-[var(--sidebar)] transition-[width] duration-150 ${
              sidebarCollapsed ? "w-[52px]" : "w-[228px]"
            }`}
          >
            <WorkspaceSwitcher collapsed={sidebarCollapsed} />

            <div className="min-h-0 flex-1 overflow-y-auto">
              <nav className="flex flex-col gap-0.5 p-2">
                {/* Cross-project active overview */}
                <button
                  type="button"
                  onClick={() => setActiveView("all-active")}
                  title={sidebarCollapsed ? t.nav.allActive : undefined}
                  className={`mb-1 flex h-[var(--row-height)] w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 text-[12.5px] transition-colors cursor-pointer ${
                    sidebarCollapsed ? "justify-center px-0" : ""
                  } ${
                    activeView === "all-active"
                      ? "bg-[var(--surface-active)] font-medium text-[var(--fg)]"
                      : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
                  }`}
                >
                  <Activity className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
                  {!sidebarCollapsed && (
                    <>
                      <span className="min-w-0 flex-1 truncate text-left">{t.nav.allActive}</span>
                      <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">
                        {allActive.length}
                      </span>
                    </>
                  )}
                </button>

                {/* 4 Project Views */}
                {NAV_ITEMS.map(({ id, icon: Icon, labelKey }) => {
                  const active = activeView === id;
                  return (
                    <button
                      key={id}
                      type="button"
                      onClick={() => setActiveView(id)}
                      title={sidebarCollapsed ? t.nav[labelKey] : undefined}
                      className={`flex h-[var(--row-height)] w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 text-[12.5px] transition-colors cursor-pointer ${
                        sidebarCollapsed ? "justify-center px-0" : ""
                      } ${
                        active
                          ? "bg-[var(--surface-active)] font-medium text-[var(--fg)]"
                          : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
                      }`}
                    >
                      <Icon className="h-3.5 w-3.5 shrink-0" />
                      {!sidebarCollapsed && (
                        <>
                          <span className="min-w-0 flex-1 truncate text-left">
                            {t.nav[labelKey]}
                          </span>
                          <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">
                            {navCounts[labelKey]}
                          </span>
                        </>
                      )}
                    </button>
                  );
                })}
              </nav>

              {/* Modules section */}
              {!sidebarCollapsed && hasReadyWorkspace && (
                <>
                  <div className="flex items-center justify-between px-4 pt-3 pb-1 text-[11px] font-medium text-[var(--fg-3)]">
                    <span>{t.nav.modules}</span>
                    <span className="font-mono-tabular">{snapshot?.modules.length ?? 0}</span>
                  </div>
                  <div className="flex flex-col gap-0.5 px-2">
                    <button
                      type="button"
                      onClick={() => setActiveModule("")}
                      className={`flex h-7 w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 text-[12px] transition-colors cursor-pointer ${
                        activeModule === ""
                          ? "bg-[var(--surface-active)] font-medium text-[var(--fg)]"
                          : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
                      }`}
                    >
                      <span className="h-1.5 w-1.5 rounded-full bg-[var(--fg-3)]" />
                      <span className="min-w-0 flex-1 truncate text-left">{t.nav.allModules}</span>
                      <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">
                        {snapshot?.stats.total ?? 0}
                      </span>
                    </button>

                    {(snapshot?.modules ?? []).map((m) => {
                      const info = resolveModule(m.key, modulePrefix, locale, moduleAliases);
                      const active = activeModule === m.key;
                      return (
                        <button
                          key={m.key}
                          type="button"
                          title={m.key}
                          onClick={() => setActiveModule(m.key)}
                          className={`flex h-7 w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 text-[12px] transition-colors cursor-pointer ${
                            active
                              ? "bg-[var(--surface-active)] font-medium text-[var(--fg)]"
                              : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
                          }`}
                        >
                          <span
                            className="h-1.5 w-1.5 shrink-0 rounded-full"
                            style={{ backgroundColor: info.color }}
                          />
                          <span className="min-w-0 flex-1 truncate text-left">{info.label}</span>
                          <span className="font-mono-tabular text-[11px] text-[var(--fg-3)]">
                            {m.count}
                          </span>
                        </button>
                      );
                    })}
                  </div>

                  {/* Active Heartbeat */}
                  <div className="flex items-center justify-between px-4 pt-4 pb-1 text-[11px] font-medium text-[var(--fg-3)]">
                    <span>{t.nav.activeNow}</span>
                    <span className="font-mono-tabular">{sidebarLiveTasks.length}</span>
                  </div>
                  <div className="flex flex-col gap-0.5 px-2 pb-3">
                    {sidebarLiveTasks.length === 0 ? (
                      <div className="px-2 py-1 text-[11.5px] text-[var(--fg-3)]">
                        {t.status.noActiveTasks}
                      </div>
                    ) : (
                      sidebarLiveTasks.slice(0, 8).map((it) => (
                        <button
                          key={it.id}
                          type="button"
                          onClick={() => openDrawer(it.id)}
                          className="flex w-full items-start gap-2 rounded-[var(--radius-sm)] px-2 py-1.5 text-left transition-colors hover:bg-[var(--surface-hover)] cursor-pointer"
                        >
                          <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--accent)]" />
                          <div className="min-w-0 flex-1">
                            <div className="line-clamp-2 text-[12px] leading-snug text-[var(--fg-2)]">
                              {it.title}
                            </div>
                            <div className="mt-0.5 font-mono-tabular text-[10px] text-[var(--fg-3)]">
                              {it.shortId}
                              {it.assignee ? ` · ${it.assignee}` : ""}
                            </div>
                          </div>
                        </button>
                      ))
                    )}
                  </div>
                </>
              )}
            </div>

            {/* Footer: Settings trigger + Live Watch LED */}
            <div className="border-t border-[var(--border)] p-1.5">
              <button
                type="button"
                onClick={() => setSettingsOpen(true)}
                title={t.actions.openSettings}
                className={`flex h-7 w-full items-center gap-2 rounded-[var(--radius-sm)] px-2 text-[11.5px] text-[var(--fg-2)] transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer ${
                  sidebarCollapsed ? "justify-center px-0" : "justify-between"
                }`}
              >
                <span className="inline-flex items-center gap-2">
                  <Settings className="h-3.5 w-3.5 shrink-0" />
                  {!sidebarCollapsed && <span>{t.nav.settings}</span>}
                </span>
                {!sidebarCollapsed && (
                  <span className="inline-flex items-center gap-1.5 font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                    <span
                      className={`h-1.5 w-1.5 rounded-full ${
                        syncStatus === "busy"
                          ? "bg-[var(--accent)]"
                          : syncStatus === "error"
                            ? "bg-[var(--danger)]"
                            : "bg-[var(--success)]"
                      }`}
                    />
                    <span>
                      {syncStatus === "busy"
                        ? t.sync.busy
                        : syncStatus === "error"
                          ? t.sync.error
                          : `v${platform.version}`}
                    </span>
                  </span>
                )}
              </button>
            </div>
          </aside>

          {/* Main Stage */}
          <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-[var(--bg)]">
            {/* Topbar */}
            <div className="flex h-11 shrink-0 items-center gap-3 border-b border-[var(--border)] px-4">
              <div className="flex min-w-0 items-center gap-2">
                <h1 className="whitespace-nowrap text-[13.5px] font-semibold text-[var(--fg)]">
                  {viewTitle}
                </h1>
                {activeModMeta && activeView !== "all-active" && (
                  <>
                    <span className="text-[var(--fg-3)]">/</span>
                    <span className="inline-flex items-center gap-1.5 text-[12px] font-medium text-[var(--fg)]">
                      <span
                        className="h-1.5 w-1.5 rounded-full"
                        style={{ backgroundColor: activeModMeta.color }}
                      />
                      <span>{activeModMeta.label}</span>
                      <button
                        type="button"
                        onClick={() => setActiveModule("")}
                        className="text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </span>
                  </>
                )}

                {/* Borderless inline KPI strip */}
                {hasReadyWorkspace && activeView !== "all-active" && (
                  <div className="ml-2 hidden items-center gap-3 border-l border-[var(--border)] pl-3 lg:flex">
                    {(
                      [
                        ["in_progress", "var(--accent)", kpiCounts.in_progress],
                        ["ready", "var(--success)", kpiCounts.ready],
                        ["blocked", "var(--danger)", kpiCounts.blocked],
                        ["closed", "var(--fg-3)", kpiCounts.closed],
                      ] as const
                    ).map(([lane, color, count]) => (
                      <button
                        key={lane}
                        type="button"
                        onClick={() => jumpToLaneList(lane)}
                        className="inline-flex items-center gap-1.5 whitespace-nowrap text-[11.5px] text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                      >
                        <span
                          className="h-1.5 w-1.5 rounded-full"
                          style={{ backgroundColor: color }}
                        />
                        <span>{t.lanes[lane]}</span>
                        <span className="font-mono-tabular font-medium text-[var(--fg-2)]">
                          {count}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </div>

              {hasReadyWorkspace && activeView !== "all-active" && (
                <div className="ml-auto flex items-center gap-1.5">
                  <div className="relative flex h-7 w-44 items-center rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2 focus-within:w-56 focus-within:border-[var(--border-strong)] transition-[width,border-color]">
                    <Search className="mr-1.5 h-3.5 w-3.5 shrink-0 text-[var(--fg-3)]" />
                    <input
                      ref={searchInputRef}
                      type="text"
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                      placeholder={t.board.searchPlaceholder}
                      className="w-full bg-transparent text-[12px] text-[var(--fg)] outline-none placeholder:text-[var(--fg-3)]"
                    />
                    {search && (
                      <button
                        type="button"
                        onClick={() => setSearch("")}
                        className="text-[var(--fg-3)] hover:text-[var(--fg)] cursor-pointer"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    )}
                  </div>

                  {activeView === "board" && (
                    <div className="inline-flex h-7 items-center rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
                      <button
                        type="button"
                        onClick={() => setBoardScope("tasks")}
                        className={`h-[22px] rounded-[var(--radius-xs)] px-2 text-[11.5px] cursor-pointer ${
                          boardScope === "tasks"
                            ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                            : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                        }`}
                      >
                        {t.board.tasksOnly}
                      </button>
                      <button
                        type="button"
                        onClick={() => setBoardScope("all")}
                        className={`h-[22px] rounded-[var(--radius-xs)] px-2 text-[11.5px] cursor-pointer ${
                          boardScope === "all"
                            ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                            : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                        }`}
                      >
                        {t.board.withEpics}
                      </button>
                    </div>
                  )}

                  <Button
                    variant="outline"
                    disabled={syncingGh}
                    onClick={() => void handleSyncGithub()}
                  >
                    <RefreshCw className={`h-3 w-3 ${syncingGh ? "animate-spin" : ""}`} />
                    <span>{t.actions.syncGithub}</span>
                  </Button>

                  <Tooltip label={t.actions.refresh}>
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label={t.actions.refresh}
                      onClick={() => void refreshSnapshot(true)}
                    >
                      <RefreshCw className="h-3.5 w-3.5" />
                    </Button>
                  </Tooltip>

                  <Button variant="primary" onClick={() => openCreateModal()}>
                    <Plus className="h-3.5 w-3.5" />
                    <span>{t.actions.newIssue}</span>
                  </Button>
                </div>
              )}
            </div>

            {/* Active Pane */}
            {activeView === "all-active" ? (
              <AllActiveView />
            ) : !activePath || !snapshot ? (
              /* Clean onboarding screen to connect first local repository */
              <div className="grid flex-1 place-items-center p-8">
                <div className="w-full max-w-[460px] rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] p-6 text-center">
                  <div className="mx-auto mb-3.5 grid h-10 w-10 place-items-center rounded-[var(--radius-md)] bg-[var(--accent-soft)] text-[var(--accent)]">
                    <FolderOpen className="h-5 w-5" />
                  </div>
                  <h2 className="text-[15px] font-semibold text-[var(--fg)]">
                    {t.workspace.welcomeTitle}
                  </h2>
                  <p className="mt-2 text-[12.5px] leading-relaxed text-[var(--fg-2)]">
                    {t.workspace.welcomeDesc}
                  </p>
                  <div className="mt-5 flex justify-center">
                    <Button variant="primary" onClick={() => void handleWelcomeBrowse()}>
                      <FolderOpen className="h-3.5 w-3.5" />
                      <span>{t.workspace.addFolder}</span>
                    </Button>
                  </div>
                  <div className="mt-4 flex items-center gap-1.5 border-t border-[var(--border-subtle)] pt-4">
                    <input
                      type="text"
                      value={welcomePath}
                      onChange={(e) => setWelcomePath(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void handleWelcomeAdd();
                      }}
                      placeholder={t.workspace.pathPlaceholder}
                      className="h-8 min-w-0 flex-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 font-mono-tabular text-[11.5px] text-[var(--fg)] focus:border-[var(--border-strong)]"
                    />
                    <Button
                      variant="outline"
                      disabled={!welcomePath.trim()}
                      onClick={() => void handleWelcomeAdd()}
                      className="h-8"
                    >
                      {t.workspace.connectBtn}
                    </Button>
                  </div>
                </div>
              </div>
            ) : !snapshot.hasBeads ? (
              <div className="grid flex-1 place-items-center p-8">
                <div className="w-full max-w-[440px] rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] p-6 text-center">
                  <h2 className="text-[15px] font-semibold text-[var(--fg)]">
                    {snapshot.repoName} · {t.workspace.uninitTitle}
                  </h2>
                  <p className="mt-2 text-[12.5px] leading-relaxed text-[var(--fg-2)]">
                    {t.workspace.uninitDesc}
                  </p>
                  <div className="mt-5 flex justify-center">
                    <Button
                      variant="primary"
                      disabled={initBusy}
                      onClick={() => void handleInitCurrent()}
                    >
                      {initBusy ? "..." : t.workspace.initCurrentBtn}
                    </Button>
                  </div>
                </div>
              </div>
            ) : activeView === "board" ? (
              <BoardView
                issues={boardIssues}
                modulePrefix={modulePrefix}
                moduleAliases={moduleAliases}
                onRequestClose={setClosingIssue}
              />
            ) : activeView === "epics" ? (
              <EpicsView
                epics={filteredEpics}
                activeModule={activeModule}
                modulePrefix={modulePrefix}
                moduleAliases={moduleAliases}
              />
            ) : activeView === "dag" ? (
              <DagView
                issues={searchScopedIssues}
                edges={snapshot.edges}
                modulePrefix={modulePrefix}
                moduleAliases={moduleAliases}
              />
            ) : (
              <ListView
                issues={searchScopedIssues}
                epics={snapshot.epics}
                modulePrefix={modulePrefix}
                moduleAliases={moduleAliases}
              />
            )}
          </main>
        </div>

        <IssueDrawer
          modulePrefix={modulePrefix}
          moduleAliases={moduleAliases}
          onRequestClose={setClosingIssue}
        />
        <CreateIssueModal modulePrefix={modulePrefix} moduleAliases={moduleAliases} />
        <CloseIssueDialog issue={closingIssue} onDone={() => setClosingIssue(null)} />
        <CommandPalette />
        <SettingsModal />
        <ToastStack />
      </div>
    </TooltipProvider>
  );
}
