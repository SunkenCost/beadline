import { Check, Copy, Monitor, Moon, RotateCcw, Sun, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Switch } from "@/components/ui/Switch";
import { useT } from "@/i18n";
import { exportCurrentThemeJson, isSafeCssColor, parseThemeJson } from "@/lib/theme";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { AppearanceMode, DensityMode, RadiusScale } from "@/types/ipc";
import { THEME_PRESETS } from "@/types/theme";

const APPEARANCE_OPTIONS: readonly { id: AppearanceMode; icon: typeof Sun }[] = [
  { id: "light", icon: Sun },
  { id: "dark", icon: Moon },
  { id: "system", icon: Monitor },
];

const RADIUS_OPTIONS: readonly RadiusScale[] = ["sharp", "default", "soft"];
const DENSITY_OPTIONS: readonly DensityMode[] = ["compact", "comfortable"];
const ACCENT_SWATCHES = ["#D97757", "#5E6AD2", "#16803C", "#0EA5E9", "#A855F7", "#E11D48"] as const;

type SettingsTab = "appearance" | "general";

export function SettingsModal() {
  const t = useT();
  const open = useUiStore((s) => s.settingsOpen);
  const setOpen = useUiStore((s) => s.setSettingsOpen);

  const {
    settings,
    setLanguage,
    setThemePreset,
    setAppearance,
    setRadius,
    setDensity,
    setAccentOverride,
    importCustomTheme,
    applyCustomTheme,
    removeCustomTheme,
    updateSettings,
  } = useSettingsStore();

  const { workspaces, activePath, updateWorkspaceConfig } = useWorkspaceStore();
  const currentWs = workspaces.find(
    (w) => activePath && w.path.toLowerCase() === activePath.toLowerCase(),
  );

  const [tab, setTab] = useState<SettingsTab>("appearance");
  const [bdPathDraft, setBdPathDraft] = useState(settings.bdPath ?? "");
  const [schemesDraft, setSchemesDraft] = useState(settings.deepLinkSchemes.join(", "));
  const [modPrefixDraft, setModPrefixDraft] = useState(currentWs?.modulePrefix ?? "mod:");
  const [themeJsonDraft, setThemeJsonDraft] = useState("");
  const [statusMsg, setStatusMsg] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  useEffect(() => {
    if (open) {
      setBdPathDraft(settings.bdPath ?? "");
      setSchemesDraft(settings.deepLinkSchemes.join(", "));
      setModPrefixDraft(currentWs?.modulePrefix ?? "mod:");
      setStatusMsg(null);
    }
  }, [open, settings.bdPath, settings.deepLinkSchemes, currentWs?.modulePrefix]);

  const showNotice = (kind: "ok" | "err", text: string) => {
    setStatusMsg({ kind, text });
    window.setTimeout(() => {
      setStatusMsg((cur) => (cur?.text === text ? null : cur));
    }, 2600);
  };

  const handleSaveGeneral = async () => {
    const schemes = schemesDraft
      .split(",")
      .map((s) =>
        s
          .trim()
          .toLowerCase()
          .replace(/:\/\/$/, ""),
      )
      .filter((s) => /^[a-z][a-z0-9+.-]*$/.test(s));
    await updateSettings({
      bdPath: bdPathDraft.trim() || null,
      deepLinkSchemes: schemes.length ? schemes : ["t3-thread", "vscode", "cursor"],
    });
    if (currentWs) {
      await updateWorkspaceConfig({
        path: currentWs.path,
        modulePrefix: modPrefixDraft.trim() || "mod:",
        moduleAliases: currentWs.moduleAliases,
        pinned: currentWs.pinned,
      });
    }
    showNotice("ok", t.settings.savedHint);
  };

  const handleExportTheme = async () => {
    const json = exportCurrentThemeJson(settings.theme);
    await navigator.clipboard?.writeText(json).catch(() => undefined);
    setThemeJsonDraft(json);
    showNotice("ok", t.theme.copiedThemeJson);
  };

  const handleImportTheme = async () => {
    const res = parseThemeJson(themeJsonDraft);
    if (!res.ok) {
      showNotice("err", res.error);
      return;
    }
    await importCustomTheme(res.theme);
    setThemeJsonDraft("");
    showNotice("ok", `${t.theme.importedTheme}: ${res.theme.name}`);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      title={t.settings.title}
      description={t.settings.subtitle}
      maxWidthClass="max-w-[620px]"
      footer={
        <div className="flex w-full items-center justify-between gap-2">
          <div className="min-h-[18px] text-[11.5px]">
            {statusMsg && (
              <span
                className={
                  statusMsg.kind === "err" ? "text-[var(--danger)]" : "text-[var(--success)]"
                }
              >
                {statusMsg.text}
              </span>
            )}
          </div>
          <Button variant="primary" onClick={() => setOpen(false)}>
            {t.actions.done}
          </Button>
        </div>
      }
    >
      {/* Clean segmented tab switcher */}
      <div className="mb-4 inline-flex rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
        <button
          type="button"
          onClick={() => setTab("appearance")}
          className={`h-7 rounded-[var(--radius-xs)] px-3 text-[12px] transition-colors cursor-pointer ${
            tab === "appearance"
              ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
              : "text-[var(--fg-2)] hover:text-[var(--fg)]"
          }`}
        >
          {t.settings.tabAppearance}
        </button>
        <button
          type="button"
          onClick={() => setTab("general")}
          className={`h-7 rounded-[var(--radius-xs)] px-3 text-[12px] transition-colors cursor-pointer ${
            tab === "general"
              ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
              : "text-[var(--fg-2)] hover:text-[var(--fg)]"
          }`}
        >
          {t.settings.tabGeneral}
        </button>
      </div>

      {tab === "appearance" ? (
        <div className="flex flex-col gap-5">
          {/* Presets */}
          <section className="flex flex-col gap-2">
            <div className="text-[11.5px] font-medium text-[var(--fg-2)]">
              {t.theme.sectionPreset}
            </div>
            <div className="grid grid-cols-3 gap-2">
              {THEME_PRESETS.map((p) => {
                const active =
                  !settings.theme.activeCustomThemeId && settings.theme.preset === p.id;
                return (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => void setThemePreset(p.id)}
                    className={`flex items-center gap-2 rounded-[var(--radius-md)] border px-3 py-2.5 text-left text-[12px] transition-colors cursor-pointer ${
                      active
                        ? "border-[var(--accent)] bg-[var(--accent-soft)] font-medium text-[var(--fg)]"
                        : "border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--fg-2)] hover:border-[var(--border-strong)] hover:text-[var(--fg)]"
                    }`}
                  >
                    <span
                      className="h-3 w-3 shrink-0 rounded-full border border-white/20"
                      style={{ backgroundColor: p.swatchAccent }}
                    />
                    <span className="truncate">{t.theme.presets[p.id]}</span>
                  </button>
                );
              })}
            </div>
          </section>

          {/* Appearance + Radius + Density */}
          <div className="grid grid-cols-3 gap-3">
            <section className="flex flex-col gap-1.5">
              <div className="text-[11.5px] font-medium text-[var(--fg-2)]">
                {t.theme.sectionAppearance}
              </div>
              <div className="grid grid-cols-3 gap-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
                {APPEARANCE_OPTIONS.map(({ id, icon: Icon }) => {
                  const active = settings.theme.appearance === id;
                  return (
                    <button
                      key={id}
                      type="button"
                      title={t.theme.appearance[id]}
                      onClick={() => void setAppearance(id)}
                      className={`flex h-7 items-center justify-center rounded-[var(--radius-xs)] text-[11px] transition-colors cursor-pointer ${
                        active
                          ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                          : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                      }`}
                    >
                      <Icon className="h-3.5 w-3.5" />
                    </button>
                  );
                })}
              </div>
            </section>

            <section className="flex flex-col gap-1.5">
              <div className="text-[11.5px] font-medium text-[var(--fg-2)]">
                {t.theme.sectionRadius}
              </div>
              <div className="grid grid-cols-3 gap-1 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
                {RADIUS_OPTIONS.map((r) => {
                  const active = settings.theme.radius === r;
                  return (
                    <button
                      key={r}
                      type="button"
                      onClick={() => void setRadius(r)}
                      className={`h-7 rounded-[var(--radius-xs)] text-[11px] transition-colors cursor-pointer ${
                        active
                          ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                          : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                      }`}
                    >
                      {t.theme.radius[r]}
                    </button>
                  );
                })}
              </div>
            </section>

            <section className="flex flex-col gap-1.5">
              <div className="text-[11.5px] font-medium text-[var(--fg-2)]">
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
                      className={`h-7 rounded-[var(--radius-xs)] text-[11px] transition-colors cursor-pointer ${
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
            </section>
          </div>

          {/* Accent color override */}
          <section className="flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="text-[11.5px] font-medium text-[var(--fg-2)]">
                {t.theme.sectionAccent}
              </span>
              {settings.theme.accentOverride && (
                <Button size="sm" variant="ghost" onClick={() => void setAccentOverride(null)}>
                  <RotateCcw className="h-3 w-3" />
                  <span>{t.actions.reset}</span>
                </Button>
              )}
            </div>
            <div className="flex items-center gap-2">
              {ACCENT_SWATCHES.map((hex) => {
                const active = settings.theme.accentOverride?.toLowerCase() === hex.toLowerCase();
                return (
                  <button
                    key={hex}
                    type="button"
                    aria-label={hex}
                    onClick={() => void setAccentOverride(hex)}
                    className="grid h-6 w-6 place-items-center rounded-full border border-white/15 transition-transform hover:scale-105 cursor-pointer"
                    style={{ backgroundColor: hex }}
                  >
                    {active && <Check className="h-3.5 w-3.5 text-white" />}
                  </button>
                );
              })}
              <input
                type="text"
                value={settings.theme.accentOverride ?? ""}
                onChange={(e) => {
                  const v = e.target.value.trim();
                  if (!v) void setAccentOverride(null);
                  else if (isSafeCssColor(v)) void setAccentOverride(v);
                }}
                placeholder="#D97757"
                className="ml-auto h-7 w-28 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2 font-mono-tabular text-[11.5px] text-[var(--fg)]"
              />
            </div>
          </section>

          {/* Custom Theme JSON Import / Export */}
          <section className="flex flex-col gap-2 border-t border-[var(--border)] pt-4">
            <div className="flex items-center justify-between">
              <span className="text-[11.5px] font-medium text-[var(--fg-2)]">
                {t.theme.sectionCustomThemes}
              </span>
              <Button size="sm" variant="outline" onClick={() => void handleExportTheme()}>
                <Copy className="h-3 w-3" />
                <span>{t.actions.exportJson}</span>
              </Button>
            </div>

            {settings.theme.customThemes.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {settings.theme.customThemes.map((ct) => {
                  const active = settings.theme.activeCustomThemeId === ct.id;
                  return (
                    <div
                      key={ct.id}
                      className={`inline-flex items-center gap-1.5 rounded-[var(--radius-xs)] border px-2 py-1 text-[11.5px] ${
                        active
                          ? "border-[var(--accent)] bg-[var(--accent-soft)] text-[var(--fg)]"
                          : "border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--fg-2)]"
                      }`}
                    >
                      <button
                        type="button"
                        onClick={() => void applyCustomTheme(ct.id)}
                        className="cursor-pointer font-medium"
                      >
                        {ct.name}
                      </button>
                      <button
                        type="button"
                        aria-label={t.actions.remove}
                        onClick={() => void removeCustomTheme(ct.id)}
                        className="text-[var(--fg-3)] hover:text-[var(--danger)] cursor-pointer"
                      >
                        <Trash2 className="h-3 w-3" />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}

            <textarea
              rows={3}
              value={themeJsonDraft}
              onChange={(e) => setThemeJsonDraft(e.target.value)}
              placeholder={t.theme.customThemePlaceholder}
              className="w-full rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-2 font-mono-tabular text-[11px] text-[var(--fg)] focus:border-[var(--border-strong)]"
            />
            <div className="flex justify-end">
              <Button
                size="sm"
                variant="outline"
                disabled={!themeJsonDraft.trim()}
                onClick={() => void handleImportTheme()}
              >
                <Upload className="h-3 w-3" />
                <span>{t.actions.importJson}</span>
              </Button>
            </div>
          </section>
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          {/* Language */}
          <div className="flex items-center justify-between gap-4 border-b border-[var(--border-subtle)] pb-4">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--fg)]">
                {t.settings.languageLabel}
              </div>
              <div className="text-[11.5px] text-[var(--fg-3)]">{t.settings.languageDesc}</div>
            </div>
            <div className="inline-flex rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
              <button
                type="button"
                onClick={() => void setLanguage("zh-CN")}
                className={`h-7 rounded-[var(--radius-xs)] px-2.5 text-[11.5px] cursor-pointer ${
                  settings.language === "zh-CN"
                    ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                    : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                }`}
              >
                简体中文
              </button>
              <button
                type="button"
                onClick={() => void setLanguage("en")}
                className={`h-7 rounded-[var(--radius-xs)] px-2.5 text-[11.5px] cursor-pointer ${
                  settings.language === "en"
                    ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                    : "text-[var(--fg-2)] hover:text-[var(--fg)]"
                }`}
              >
                English
              </button>
            </div>
          </div>

          {/* Force Writes */}
          <div className="flex items-center justify-between gap-4 border-b border-[var(--border-subtle)] pb-4">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--fg)]">
                {t.settings.forceWritesLabel}
              </div>
              <div className="text-[11.5px] text-[var(--fg-3)]">{t.settings.forceWritesDesc}</div>
            </div>
            <Switch
              checked={settings.useForceWrites}
              onCheckedChange={(checked) => void updateSettings({ useForceWrites: checked })}
              ariaLabel={t.settings.forceWritesLabel}
            />
          </div>

          {/* bd Path */}
          <div className="flex flex-col gap-1.5">
            <label htmlFor="bd-path-input" className="text-[12px] font-medium text-[var(--fg)]">
              {t.settings.bdPathLabel}
            </label>
            <input
              id="bd-path-input"
              type="text"
              value={bdPathDraft}
              onChange={(e) => setBdPathDraft(e.target.value)}
              placeholder={t.settings.bdPathPlaceholder}
              className="h-8 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 font-mono-tabular text-[12px] text-[var(--fg)] focus:border-[var(--border-strong)]"
            />
          </div>

          {/* Per-workspace Module Prefix */}
          {currentWs && (
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="mod-prefix-input"
                className="text-[12px] font-medium text-[var(--fg)]"
              >
                {t.settings.modulePrefixLabel} ({currentWs.name})
              </label>
              <input
                id="mod-prefix-input"
                type="text"
                value={modPrefixDraft}
                onChange={(e) => setModPrefixDraft(e.target.value)}
                placeholder="mod:"
                className="h-8 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 font-mono-tabular text-[12px] text-[var(--fg)] focus:border-[var(--border-strong)]"
              />
              <span className="text-[11px] text-[var(--fg-3)]">{t.settings.modulePrefixDesc}</span>
            </div>
          )}

          {/* Deep Link Schemes */}
          <div className="flex flex-col gap-1.5">
            <label
              htmlFor="deeplink-schemes-input"
              className="text-[12px] font-medium text-[var(--fg)]"
            >
              {t.settings.deepLinksLabel}
            </label>
            <input
              id="deeplink-schemes-input"
              type="text"
              value={schemesDraft}
              onChange={(e) => setSchemesDraft(e.target.value)}
              className="h-8 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] px-2.5 font-mono-tabular text-[12px] text-[var(--fg)] focus:border-[var(--border-strong)]"
            />
            <span className="text-[11px] text-[var(--fg-3)]">{t.settings.deepLinksDesc}</span>
          </div>

          <div className="flex justify-end pt-1">
            <Button variant="primary" onClick={() => void handleSaveGeneral()}>
              {t.actions.save}
            </Button>
          </div>
        </div>
      )}
    </Dialog>
  );
}
