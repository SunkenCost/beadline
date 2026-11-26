import { Activity, GitFork, Kanban, Layers, List, Settings } from "lucide-react";
import { useEffect } from "react";
import { Titlebar } from "@/components/shell/Titlebar";
import { TooltipProvider } from "@/components/ui/Tooltip";
import { useT } from "@/i18n";
import { applyThemeToDocument } from "@/lib/theme";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { type ActiveView, useUiStore } from "@/stores/useUiStore";

const NAV_ITEMS: readonly {
  id: ActiveView;
  icon: typeof Kanban;
  labelKey: "allActive" | "board" | "epics" | "dag" | "list";
}[] = [
  { id: "all-active", icon: Activity, labelKey: "allActive" },
  { id: "board", icon: Kanban, labelKey: "board" },
  { id: "epics", icon: Layers, labelKey: "epics" },
  { id: "dag", icon: GitFork, labelKey: "dag" },
  { id: "list", icon: List, labelKey: "list" },
];

export function App() {
  const t = useT();
  const hydrate = useSettingsStore((s) => s.hydrate);
  const theme = useSettingsStore((s) => s.settings.theme);
  const platform = useSettingsStore((s) => s.platform);
  const { activeView, sidebarCollapsed, setActiveView } = useUiStore();

  useEffect(() => {
    void hydrate();
  }, [hydrate]);

  // Keep `system` appearance mode in sync with OS light/dark changes
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const sync = () => applyThemeToDocument(theme, mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, [theme]);

  return (
    <TooltipProvider delay={300}>
      <div className="flex h-screen w-screen flex-col overflow-hidden bg-[var(--bg)] text-[var(--fg)]">
        <Titlebar />

        <div className="flex min-h-0 flex-1 overflow-hidden">
          {/* Collapsible Minimal Sidebar */}
          <aside
            className={`flex shrink-0 flex-col border-r border-[var(--border)] bg-[var(--sidebar)] transition-[width] duration-150 ${
              sidebarCollapsed ? "w-[52px]" : "w-[224px]"
            }`}
          >
            <nav className="flex flex-1 flex-col gap-0.5 p-2">
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
                    {!sidebarCollapsed && <span className="truncate">{t.nav[labelKey]}</span>}
                  </button>
                );
              })}
            </nav>

            <div
              className={`flex items-center gap-2 border-t border-[var(--border)] px-3 py-2 text-[11px] text-[var(--fg-3)] ${
                sidebarCollapsed ? "justify-center px-0" : "justify-between"
              }`}
            >
              <div className="flex items-center gap-2">
                <span className="h-1.5 w-1.5 rounded-full bg-[var(--success)]" />
                {!sidebarCollapsed && <span>{t.status.engineStandby}</span>}
              </div>
              {!sidebarCollapsed && (
                <span className="inline-flex items-center gap-1 font-mono-tabular text-[10.5px]">
                  <Settings className="h-3 w-3" />v{platform.version}
                </span>
              )}
            </div>
          </aside>

          {/* Main Stage */}
          <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-[var(--bg)]">
            <div className="flex h-11 shrink-0 items-center justify-between border-b border-[var(--border)] px-4">
              <h1 className="text-[13.5px] font-semibold text-[var(--fg)]">
                {t.nav[NAV_ITEMS.find((n) => n.id === activeView)?.labelKey ?? "board"]}
              </h1>
              <span className="text-[11.5px] text-[var(--fg-3)]">{t.status.milestoneHint}</span>
            </div>

            <div className="grid flex-1 place-items-center p-8">
              <div className="max-w-[480px] text-center">
                <div className="mx-auto mb-4 grid h-10 w-10 place-items-center rounded-[var(--radius-md)] bg-[var(--accent-soft)] text-[var(--accent)]">
                  <Kanban className="h-5 w-5" />
                </div>
                <h2 className="text-[15px] font-semibold text-[var(--fg)]">
                  {t.status.desktopReadyTitle}
                </h2>
                <p className="mt-2 text-[12.5px] leading-relaxed text-[var(--fg-2)]">
                  {t.status.desktopReadyDesc}
                </p>
              </div>
            </div>
          </main>
        </div>
      </div>
    </TooltipProvider>
  );
}
