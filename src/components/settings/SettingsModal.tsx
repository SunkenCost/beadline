import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Copy,
  Cpu,
  Download,
  Monitor,
  Moon,
  Plug,
  RotateCcw,
  Stethoscope,
  Sun,
  Trash2,
  Upload,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Switch } from "@/components/ui/Switch";
import { useT } from "@/i18n";
import {
  formatIpcError,
  ipcGetBdStatus,
  ipcGetBlStatus,
  ipcInstallManagedBd,
  ipcRunWorkspaceDoctor,
} from "@/lib/ipc";
import { buildHarnessSnippets, type HarnessId } from "@/lib/mcp-snippets";
import { resolveModule } from "@/lib/modules";
import { exportCurrentThemeJson, isSafeCssColor, parseThemeJson } from "@/lib/theme";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type {
  AppearanceMode,
  BlCliStatus,
  DensityMode,
  DoctorReport,
  ManagedBdStatus,
  ModuleAliasConfig,
  RadiusScale,
} from "@/types/ipc";
import { THEME_PRESETS } from "@/types/theme";

const APPEARANCE_OPTIONS: readonly { id: AppearanceMode; icon: typeof Sun }[] = [
  { id: "light", icon: Sun },
  { id: "dark", icon: Moon },
  { id: "system", icon: Monitor },
];

const RADIUS_OPTIONS: readonly RadiusScale[] = ["sharp", "default", "soft"];
const DENSITY_OPTIONS: readonly DensityMode[] = ["compact", "comfortable"];
const ACCENT_SWATCHES = ["#D97757", "#5E6AD2", "#16803C", "#0EA5E9", "#A855F7", "#E11D48"] as const;
const MODULE_COLORS = [
  "#D97757",
  "#6EA1E8",
  "#A78BFA",
  "#46B574",
  "#38BDF8",
  "#E0A84B",
  "#F472B6",
  "#94A3B8",
] as const;

type SettingsTab = "appearance" | "general" | "ai-setup";

export function SettingsModal() {
  const t = useT();
  const open = useUiStore((s) => s.settingsOpen);
  const setOpen = useUiStore((s) => s.setSettingsOpen);

  const {
    settings,
    platform,
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

  const { workspaces, activePath, snapshot, updateWorkspaceConfig } = useWorkspaceStore();
  const currentWs = workspaces.find(
    (w) => activePath && w.path.toLowerCase() === activePath.toLowerCase(),
  );

  const [tab, setTab] = useState<SettingsTab>("appearance");
  const [bdPathDraft, setBdPathDraft] = useState(settings.bdPath ?? "");
  const [bdStatus, setBdStatus] = useState<ManagedBdStatus | null>(null);
  const [blStatus, setBlStatus] = useState<BlCliStatus | null>(null);
  const [doctorReport, setDoctorReport] = useState<DoctorReport | null>(null);
  const [doctorBusy, setDoctorBusy] = useState(false);
  const [selectedHarness, setSelectedHarness] = useState<HarnessId>("claude-code");
  const [installingBd, setInstallingBd] = useState(false);
  const [schemesDraft, setSchemesDraft] = useState(settings.deepLinkSchemes.join(", "));
  const [modPrefixDraft, setModPrefixDraft] = useState(currentWs?.modulePrefix ?? "mod:");
  const [aliasesDraft, setAliasesDraft] = useState<Record<string, ModuleAliasConfig>>({});
  const [themeJsonDraft, setThemeJsonDraft] = useState("");
  const [statusMsg, setStatusMsg] = useState<{ kind: "ok" | "err"; text: string } | null>(null);

  useEffect(() => {
    if (!open) return;
    const ws = useWorkspaceStore
      .getState()
      .workspaces.find((w) => activePath && w.path.toLowerCase() === activePath.toLowerCase());
    setBdPathDraft(settings.bdPath ?? "");
    setSchemesDraft(settings.deepLinkSchemes.join(", "));
    setModPrefixDraft(ws?.modulePrefix ?? "mod:");
    setAliasesDraft({ ...(ws?.moduleAliases ?? {}) });
    setStatusMsg(null);
    void ipcGetBdStatus()
      .then((st) => setBdStatus(st))
      .catch(() => setBdStatus({ installed: false, path: null, source: null, version: null }));
    void ipcGetBlStatus()
      .then((st) => setBlStatus(st))
      .catch(() => setBlStatus({ installed: false, path: null, version: null }));
    setDoctorReport(null);
  }, [open, activePath, settings.bdPath, settings.deepLinkSchemes]);

  const harnessSnippets = useMemo(
    () => buildHarnessSnippets(blStatus?.path ?? null, activePath, platform.os),
    [blStatus?.path, activePath, platform.os],
  );
  const activeSnippet = harnessSnippets.find((s) => s.id === selectedHarness) ?? harnessSnippets[0];

  const handleRunDoctor = async () => {
    if (!activePath) return;
    setDoctorBusy(true);
    try {
      const rep = await ipcRunWorkspaceDoctor(activePath);
      setDoctorReport(rep);
    } catch (err) {
      showNotice("err", formatIpcError(err));
    } finally {
      setDoctorBusy(false);
    }
  };

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
      const cleanedAliases: Record<string, ModuleAliasConfig> = {};
      for (const [k, v] of Object.entries(aliasesDraft)) {
        const lbl = v.label?.trim() || null;
        const col = v.color?.trim() || null;
        if (lbl || col) {
          cleanedAliases[k] = { label: lbl, color: col };
        }
      }
      await updateWorkspaceConfig({
        path: currentWs.path,
        modulePrefix: modPrefixDraft.trim() || "mod:",
        moduleAliases: cleanedAliases,
        pinned: currentWs.pinned,
      });
    }
    const refreshed = await ipcGetBdStatus().catch(() => null);
    if (refreshed) setBdStatus(refreshed);
    showNotice("ok", t.settings.savedHint);
  };

  const handleInstallManagedBd = async () => {
    setInstallingBd(true);
    try {
      const installed = await ipcInstallManagedBd();
      setBdStatus(installed);
      showNotice("ok", t.settings.installedBdToast);
    } catch (err) {
      showNotice("err", formatIpcError(err));
    } finally {
      setInstallingBd(false);
    }
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

  const installCmd =
    platform.os === "windows" ? "winget install -e --id GasTownHall.Beads" : "brew install beads";

  const activeModules = snapshot?.modules ?? [];

  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      title={t.settings.title}
      description={t.settings.subtitle}
      maxWidthClass="max-w-[640px]"
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
        <button
          type="button"
          onClick={() => setTab("ai-setup")}
          className={`inline-flex h-7 items-center gap-1.5 rounded-[var(--radius-xs)] px-3 text-[12px] transition-colors cursor-pointer ${
            tab === "ai-setup"
              ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
              : "text-[var(--fg-2)] hover:text-[var(--fg)]"
          }`}
        >
          <Plug className="h-3 w-3 text-[var(--accent)]" />
          <span>{t.settings.tabAiSetup}</span>
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
      ) : tab === "ai-setup" ? (
        <div className="flex flex-col gap-4">
          {/* bl Binary Detection & Workspace Doctor */}
          <div className="space-y-2.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] px-3.5 py-3">
            <div className="flex items-center justify-between gap-2">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--fg)]">
                  {blStatus?.installed ? (
                    <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-[var(--success)]" />
                  ) : (
                    <AlertTriangle className="h-3.5 w-3.5 shrink-0 text-[var(--warning)]" />
                  )}
                  <span>
                    {t.settings.blStatusLabel} ·{" "}
                    {blStatus?.installed ? t.settings.blDetected : t.settings.blMissing}
                  </span>
                </div>
                <div className="mt-0.5 truncate font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                  {blStatus?.installed
                    ? `${blStatus.version ? `${blStatus.version} · ` : ""}${blStatus.path ?? ""}`
                    : t.settings.blMissingHint}
                </div>
              </div>
              {activePath && (
                <Button
                  size="sm"
                  variant="outline"
                  disabled={doctorBusy}
                  onClick={() => void handleRunDoctor()}
                >
                  <Stethoscope className="h-3 w-3" />
                  <span>{t.settings.doctorBtn}</span>
                </Button>
              )}
            </div>

            {doctorReport && (
              <div className="space-y-1 border-t border-[var(--border-subtle)] pt-2 text-[11px]">
                <div
                  className={`font-medium ${
                    doctorReport.ok ? "text-[var(--success)]" : "text-[var(--warning)]"
                  }`}
                >
                  {doctorReport.ok ? t.settings.doctorOk : t.settings.doctorWarn}
                </div>
                <div className="font-mono-tabular text-[10.5px] text-[var(--fg-2)]">
                  store: {doctorReport.storeDir} ({doctorReport.issueCount} issues ·{" "}
                  {doctorReport.activeSessions} active sessions)
                </div>
                {doctorReport.warnings.map((w) => (
                  <div key={w} className="text-[var(--warning)]">
                    · {w}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Harness MCP & CLI Snippets */}
          <div className="space-y-2.5">
            <div>
              <div className="text-[12.5px] font-medium text-[var(--fg)]">
                {t.settings.mcpSnippetsTitle}
              </div>
              <div className="mt-0.5 text-[11.5px] text-[var(--fg-3)]">
                {t.settings.mcpSnippetsDesc}
              </div>
            </div>

            <div className="flex flex-wrap gap-1.5">
              {harnessSnippets.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => setSelectedHarness(item.id)}
                  className={`h-7 rounded-[var(--radius-xs)] border px-2.5 text-[11.5px] transition-colors cursor-pointer ${
                    selectedHarness === item.id
                      ? "border-[var(--accent)] bg-[var(--accent-soft)] font-medium text-[var(--fg)]"
                      : "border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--fg-2)] hover:text-[var(--fg)]"
                  }`}
                >
                  {item.name}
                </button>
              ))}
            </div>

            {activeSnippet && (
              <div className="space-y-3">
                {activeSnippet.hookCode && activeSnippet.hookTargetFile && (
                  <div className="space-y-2 rounded-[var(--radius-md)] border border-[var(--accent-border)] bg-[var(--accent-soft)]/25 p-3">
                    <div className="flex items-center justify-between gap-2">
                      <div className="min-w-0">
                        <div className="text-[12px] font-medium text-[var(--fg)]">
                          {t.settings.hookSectionTitle}
                        </div>
                        <div className="font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                          {activeSnippet.hookTargetFile}
                        </div>
                      </div>
                      <Button
                        size="sm"
                        variant="primary"
                        onClick={() => {
                          void navigator.clipboard?.writeText(activeSnippet.hookCode ?? "");
                          showNotice("ok", t.settings.copiedConfig);
                        }}
                      >
                        <Copy className="h-3 w-3" />
                        <span>{t.settings.copyConfig}</span>
                      </Button>
                    </div>
                    <p className="text-[11px] leading-relaxed text-[var(--fg-2)]">
                      {t.settings.hookSectionDesc}
                    </p>
                    <pre className="max-h-40 overflow-auto rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-[var(--surface)] p-2.5 font-mono-tabular text-[11px] leading-relaxed text-[var(--fg)] select-text">
                      {activeSnippet.hookCode}
                    </pre>
                    {activeSnippet.hookNote === "codex-trust" && (
                      <p className="text-[11px] leading-relaxed text-[var(--warning)]">
                        {t.settings.codexHookTrustNote}
                      </p>
                    )}
                  </div>
                )}

                <div className="space-y-2 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] p-3">
                  <div className="flex items-center justify-between gap-2">
                    <div className="min-w-0">
                      <div className="text-[12px] font-medium text-[var(--fg)]">
                        {activeSnippet.id === "t3-cli"
                          ? t.settings.cliFallbackTitle
                          : t.settings.mcpSectionTitle}
                      </div>
                      <span className="font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                        {activeSnippet.targetFile}
                      </span>
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        void navigator.clipboard?.writeText(activeSnippet.code);
                        showNotice("ok", t.settings.copiedConfig);
                      }}
                    >
                      <Copy className="h-3 w-3" />
                      <span>{t.settings.copyConfig}</span>
                    </Button>
                  </div>
                  <pre className="overflow-x-auto rounded-[var(--radius-sm)] border border-[var(--border-subtle)] bg-[var(--surface)] p-2.5 font-mono-tabular text-[11px] leading-relaxed text-[var(--fg)] select-text">
                    {activeSnippet.code}
                  </pre>
                  {activeSnippet.id === "t3-cli" && (
                    <p className="text-[11px] leading-relaxed text-[var(--fg-3)]">
                      {t.settings.t3Note}
                    </p>
                  )}
                </div>
              </div>
            )}
          </div>
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

          {/* Dual-Mode Engine & bd CLI Health Banner */}
          <div className="space-y-2.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] px-3.5 py-3">
            {snapshot?.hasBeads && (
              <div className="flex items-center justify-between gap-2 border-b border-[var(--border-subtle)] pb-2">
                <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--fg)]">
                  <Cpu className="h-3.5 w-3.5 shrink-0 text-[var(--accent)]" />
                  <span>{t.settings.engineCurrentLabel}</span>
                </div>
                <span className="font-mono-tabular text-[11px] text-[var(--fg-2)]">
                  {snapshot.storageBackend === "native-jsonl"
                    ? t.settings.engineNativeDetail
                    : t.settings.engineDoltDetail}
                </span>
              </div>
            )}

            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5 text-[12px] font-medium text-[var(--fg)]">
                  <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-[var(--success)]" />
                  <span>
                    {t.settings.bdStatusLabel} ·{" "}
                    {bdStatus?.installed ? t.settings.bdDetected : t.settings.bdMissing}
                  </span>
                  {bdStatus?.installed && bdStatus.source && (
                    <span className="rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-1.5 py-0.5 font-mono-tabular text-[10px] text-[var(--fg-3)]">
                      {bdStatus.source === "managed"
                        ? t.settings.bdSourceManaged
                        : bdStatus.source === "custom"
                          ? t.settings.bdSourceCustom
                          : t.settings.bdSourceSystem}
                    </span>
                  )}
                </div>
                <div className="mt-0.5 truncate font-mono-tabular text-[10.5px] text-[var(--fg-3)]">
                  {bdStatus?.installed
                    ? `${bdStatus.version ? `${bdStatus.version} · ` : ""}${bdStatus.path ?? ""}`
                    : t.settings.bdMissingHint}
                </div>
              </div>
              {!bdStatus?.installed && (
                <div className="flex shrink-0 items-center gap-1.5">
                  <Button
                    size="sm"
                    variant="primary"
                    disabled={installingBd}
                    onClick={() => void handleInstallManagedBd()}
                  >
                    <Download className="h-3 w-3" />
                    <span>
                      {installingBd ? t.settings.installingBdBtn : t.settings.installBdBtn}
                    </span>
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      void navigator.clipboard?.writeText(installCmd);
                      showNotice("ok", t.settings.copiedInstallCmd);
                    }}
                  >
                    <Copy className="h-3 w-3" />
                  </Button>
                </div>
              )}
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

          {/* Per-workspace Module Prefix & Aliases */}
          {currentWs && (
            <div className="space-y-3 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-subtle)] p-3">
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
                  className="h-8 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] px-2.5 font-mono-tabular text-[12px] text-[var(--fg)] focus:border-[var(--border-strong)]"
                />
                <span className="text-[11px] text-[var(--fg-3)]">
                  {t.settings.modulePrefixDesc}
                </span>
              </div>

              <div className="space-y-1.5 border-t border-[var(--border-subtle)] pt-2.5">
                <div className="text-[12px] font-medium text-[var(--fg)]">
                  {t.settings.moduleAliasesLabel}
                </div>
                <div className="text-[11px] text-[var(--fg-3)]">{t.settings.moduleAliasesDesc}</div>
                {activeModules.length === 0 ? (
                  <div className="text-[11.5px] text-[var(--fg-3)]">
                    {t.settings.moduleAliasesEmpty}
                  </div>
                ) : (
                  <div className="space-y-1.5 pt-1">
                    {activeModules.map((m) => {
                      const resolved = resolveModule(
                        m.key,
                        modPrefixDraft || "mod:",
                        settings.language,
                        aliasesDraft,
                      );
                      const cur = aliasesDraft[m.key] ?? {};
                      return (
                        <div key={m.key} className="flex items-center gap-2">
                          <span className="w-28 shrink-0 truncate font-mono-tabular text-[11px] text-[var(--fg-3)]">
                            {m.key}
                          </span>
                          <input
                            type="text"
                            value={cur.label ?? ""}
                            placeholder={resolved.label}
                            onChange={(e) =>
                              setAliasesDraft((prev) => ({
                                ...prev,
                                [m.key]: { ...prev[m.key], label: e.target.value },
                              }))
                            }
                            className="h-7 min-w-0 flex-1 rounded-[var(--radius-xs)] border border-[var(--border)] bg-[var(--surface)] px-2 text-[12px] text-[var(--fg)]"
                          />
                          <div className="flex items-center gap-1">
                            {MODULE_COLORS.slice(0, 6).map((hex) => {
                              const picked =
                                (cur.color ?? resolved.color).toLowerCase() === hex.toLowerCase();
                              return (
                                <button
                                  key={hex}
                                  type="button"
                                  aria-label={hex}
                                  onClick={() =>
                                    setAliasesDraft((prev) => ({
                                      ...prev,
                                      [m.key]: { ...prev[m.key], color: hex },
                                    }))
                                  }
                                  className={`h-4 w-4 rounded-full cursor-pointer transition-transform ${
                                    picked
                                      ? "scale-115 ring-1 ring-[var(--fg)] ring-offset-1 ring-offset-[var(--surface-subtle)]"
                                      : "opacity-75 hover:opacity-100"
                                  }`}
                                  style={{ backgroundColor: hex }}
                                />
                              );
                            })}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
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
