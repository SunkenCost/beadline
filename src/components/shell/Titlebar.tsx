import {
  Command,
  Languages,
  Minus,
  Monitor,
  Moon,
  Palette,
  PanelLeft,
  Settings,
  Square,
  Sun,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Popover } from "@/components/ui/Popover";
import { Tooltip } from "@/components/ui/Tooltip";
import { useWindowState } from "@/hooks/useWindowState";
import { useT } from "@/i18n";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import type { AppearanceMode, DensityMode } from "@/types/ipc";
import { THEME_PRESETS } from "@/types/theme";

const APPEARANCE_OPTIONS: readonly {
  id: AppearanceMode;
  icon: typeof Sun;
}[] = [
  { id: "light", icon: Sun },
  { id: "dark", icon: Moon },
  { id: "system", icon: Monitor },
];

const DENSITY_OPTIONS: readonly DensityMode[] = ["compact", "comfortable"];

export function Titlebar() {
  const t = useT();
  const { maximized, minimize, toggleMaximize, close } = useWindowState();
  const { settings, platform, setLanguage, setThemePreset, setAppearance, setDensity } =
    useSettingsStore();
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const setSettingsOpen = useUiStore((s) => s.setSettingsOpen);
  const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);

  const isMac = platform.os === "macos";

  return (
    <header className="flex h-10 shrink-0 items-center border-b border-[var(--border)] bg-[var(--sidebar)] select-none">
      {/* Left: non-interactive brand area (draggable) + sidebar toggle button */}
      <div className={`flex h-full items-center gap-2 px-3 ${isMac ? "pl-[76px]" : ""}`}>
        <div data-tauri-drag-region className="flex h-full items-center gap-2 pr-1 cursor-default">
          <span
            aria-hidden="true"
            className="grid h-4.5 w-4.5 place-items-center rounded-[4px] bg-[var(--accent)] text-white pointer-events-none"
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path
                d="M7.5 8 C7.5 13 12 11.5 12 16 M16.5 8 C16.5 13 12 11.5 12 16"
                stroke="currentColor"
                strokeWidth="2.4"
                strokeLinecap="round"
              />
              <circle cx="7.5" cy="7.5" r="2.6" fill="currentColor" />
              <circle cx="16.5" cy="7.5" r="2.6" fill="currentColor" fillOpacity="0.85" />
              <circle cx="12" cy="17" r="2.8" fill="currentColor" />
            </svg>
          </span>
          <span className="text-[12.5px] font-semibold tracking-tight text-[var(--fg)] pointer-events-none">
            {t.appName}
          </span>
        </div>

        <Tooltip label={t.actions.collapseSidebar}>
          <Button
            size="icon"
            variant="ghost"
            aria-label={t.actions.collapseSidebar}
            onClick={toggleSidebar}
            className="h-6 w-6"
          >
            <PanelLeft className="h-3.5 w-3.5" />
          </Button>
        </Tooltip>
      </div>

      {/* Dedicated center drag spacer — only this & brand area carry data-tauri-drag-region */}
      <div className="h-full flex-1" data-tauri-drag-region />

      {/* Right: Command Palette + Language + Theme Popover + Settings + Native Caption Controls */}
      <div className="flex items-center gap-1 pr-1.5">
        <Tooltip label={t.actions.commandPalette}>
          <Button
            size="sm"
            variant="ghost"
            aria-label={t.actions.commandPalette}
            onClick={() => setPaletteOpen(true)}
            className="font-mono-tabular text-[11px] text-[var(--fg-3)]"
          >
            <Command className="h-3 w-3" />
            <span>{isMac ? "⌘K" : "Ctrl K"}</span>
          </Button>
        </Tooltip>

        <Tooltip label={t.actions.switchLanguage}>
          <Button
            size="sm"
            variant="ghost"
            aria-label={t.actions.switchLanguage}
            onClick={() => void setLanguage(settings.language === "zh-CN" ? "en" : "zh-CN")}
            className="font-mono-tabular text-[11px]"
          >
            <Languages className="h-3.5 w-3.5" />
            <span>{settings.language === "zh-CN" ? "中文" : "EN"}</span>
          </Button>
        </Tooltip>

        <Popover
          align="end"
          className="w-[248px] p-2.5"
          trigger={
            <Button
              size="icon"
              variant="ghost"
              aria-label={t.actions.switchTheme}
              title={t.actions.switchTheme}
              className="h-6 w-6"
            >
              <Palette className="h-3.5 w-3.5" />
            </Button>
          }
        >
          <div className="flex flex-col gap-3">
            <div>
              <div className="mb-1.5 px-1 text-[11px] font-medium text-[var(--fg-3)]">
                {t.theme.sectionPreset}
              </div>
              <div className="flex flex-col gap-0.5">
                {THEME_PRESETS.map((p) => {
                  const active =
                    !settings.theme.activeCustomThemeId && settings.theme.preset === p.id;
                  return (
                    <button
                      key={p.id}
                      type="button"
                      onClick={() => void setThemePreset(p.id)}
                      className={`flex h-7 w-full items-center gap-2 rounded-[var(--radius-xs)] px-2 text-left text-[12px] transition-colors cursor-pointer ${
                        active
                          ? "bg-[var(--surface-active)] font-medium text-[var(--fg)]"
                          : "text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)]"
                      }`}
                    >
                      <span
                        className="h-2.5 w-2.5 rounded-full border border-white/20"
                        style={{ backgroundColor: p.swatchAccent }}
                      />
                      <span>{t.theme.presets[p.id]}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div>
              <div className="mb-1.5 px-1 text-[11px] font-medium text-[var(--fg-3)]">
                {t.theme.sectionAppearance}
              </div>
              <div className="grid grid-cols-3 gap-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
                {APPEARANCE_OPTIONS.map(({ id, icon: Icon }) => {
                  const active = settings.theme.appearance === id;
                  return (
                    <button
                      key={id}
                      type="button"
                      onClick={() => void setAppearance(id)}
                      className={`flex h-6 items-center justify-center gap-1 rounded-[var(--radius-xs)] text-[11px] transition-colors cursor-pointer ${
                        active
                          ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                          : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                      }`}
                    >
                      <Icon className="h-3 w-3" />
                      <span>{t.theme.appearance[id]}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div>
              <div className="mb-1.5 px-1 text-[11px] font-medium text-[var(--fg-3)]">
                {t.theme.sectionDensity}
              </div>
              <div className="grid grid-cols-2 gap-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
                {DENSITY_OPTIONS.map((d) => {
                  const active = settings.theme.density === d;
                  return (
                    <button
                      key={d}
                      type="button"
                      onClick={() => void setDensity(d)}
                      className={`h-6 rounded-[var(--radius-xs)] text-[11px] transition-colors cursor-pointer ${
                        active
                          ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                          : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                      }`}
                    >
                      {t.theme.density[d]}
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </Popover>

        <Tooltip label={t.actions.openSettings}>
          <Button
            size="icon"
            variant="ghost"
            aria-label={t.actions.openSettings}
            onClick={() => setSettingsOpen(true)}
            className="h-6 w-6"
          >
            <Settings className="h-3.5 w-3.5" />
          </Button>
        </Tooltip>

        {!isMac && (
          <div className="ml-1.5 flex items-center border-l border-[var(--border)] pl-1.5">
            <button
              type="button"
              aria-label={t.actions.minimize}
              onClick={minimize}
              className="grid h-7 w-8 place-items-center rounded-[var(--radius-xs)] text-[var(--fg-2)] transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
            >
              <Minus className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              aria-label={maximized ? t.actions.restore : t.actions.maximize}
              onClick={toggleMaximize}
              className="grid h-7 w-8 place-items-center rounded-[var(--radius-xs)] text-[var(--fg-2)] transition-colors hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
            >
              <Square className="h-3 w-3" />
            </button>
            <button
              type="button"
              aria-label={t.actions.close}
              onClick={close}
              className="grid h-7 w-8 place-items-center rounded-[var(--radius-xs)] text-[var(--fg-2)] transition-colors hover:bg-[var(--danger)] hover:text-white cursor-pointer"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}
      </div>
    </header>
  );
}
