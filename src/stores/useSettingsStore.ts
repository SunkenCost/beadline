import { create } from "zustand";
import { DEFAULT_SETTINGS, ipcGetPlatformInfo, ipcGetSettings, ipcUpdateSettings } from "@/lib/ipc";
import { applyThemeToDocument } from "@/lib/theme";
import type {
  AppearanceMode,
  AppSettings,
  BeadlineThemeV1,
  DensityMode,
  Locale,
  PlatformInfo,
  RadiusScale,
  ThemePresetId,
} from "@/types/ipc";

interface SettingsState {
  settings: AppSettings;
  platform: PlatformInfo;
  loaded: boolean;
  hydrate: () => Promise<void>;
  setLanguage: (language: Locale) => Promise<void>;
  setThemePreset: (preset: ThemePresetId) => Promise<void>;
  setAppearance: (appearance: AppearanceMode) => Promise<void>;
  setRadius: (radius: RadiusScale) => Promise<void>;
  setDensity: (density: DensityMode) => Promise<void>;
  setAccentOverride: (accentOverride: string | null) => Promise<void>;
  importCustomTheme: (theme: BeadlineThemeV1) => Promise<void>;
  applyCustomTheme: (id: string | null) => Promise<void>;
  removeCustomTheme: (id: string) => Promise<void>;
  updateSettings: (patch: Partial<AppSettings>) => Promise<void>;
}

function prefersDarkNow(): boolean {
  return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  platform: { os: "windows", arch: "x86_64", version: "0.1.0" },
  loaded: false,

  hydrate: async () => {
    const [settings, platform] = await Promise.all([
      ipcGetSettings().catch(() => DEFAULT_SETTINGS),
      ipcGetPlatformInfo().catch(() => ({ os: "web", arch: "browser", version: "0.1.0" })),
    ]);
    applyThemeToDocument(settings.theme, prefersDarkNow());
    document.documentElement.lang = settings.language;
    set({ settings, platform, loaded: true });
  },

  updateSettings: async (patch) => {
    const next: AppSettings = { ...get().settings, ...patch };
    applyThemeToDocument(next.theme, prefersDarkNow());
    document.documentElement.lang = next.language;
    set({ settings: next });
    const persisted = await ipcUpdateSettings(next).catch(() => next);
    set({ settings: persisted });
  },

  setLanguage: async (language) => {
    await get().updateSettings({ language });
  },

  setThemePreset: async (preset) => {
    const cur = get().settings;
    await get().updateSettings({
      theme: { ...cur.theme, preset, activeCustomThemeId: null },
    });
  },

  setAppearance: async (appearance) => {
    const cur = get().settings;
    await get().updateSettings({
      theme: { ...cur.theme, appearance },
    });
  },

  setRadius: async (radius) => {
    const cur = get().settings;
    await get().updateSettings({
      theme: { ...cur.theme, radius },
    });
  },

  setDensity: async (density) => {
    const cur = get().settings;
    await get().updateSettings({
      theme: { ...cur.theme, density },
    });
  },

  setAccentOverride: async (accentOverride) => {
    const cur = get().settings;
    await get().updateSettings({
      theme: { ...cur.theme, accentOverride },
    });
  },

  importCustomTheme: async (theme) => {
    const cur = get().settings;
    const filtered = cur.theme.customThemes.filter((t) => t.id !== theme.id);
    await get().updateSettings({
      theme: {
        ...cur.theme,
        customThemes: [...filtered, theme],
        activeCustomThemeId: theme.id,
      },
    });
  },

  applyCustomTheme: async (id) => {
    const cur = get().settings;
    await get().updateSettings({
      theme: { ...cur.theme, activeCustomThemeId: id },
    });
  },

  removeCustomTheme: async (id) => {
    const cur = get().settings;
    const nextThemes = cur.theme.customThemes.filter((t) => t.id !== id);
    const nextActive = cur.theme.activeCustomThemeId === id ? null : cur.theme.activeCustomThemeId;
    await get().updateSettings({
      theme: {
        ...cur.theme,
        customThemes: nextThemes,
        activeCustomThemeId: nextActive,
      },
    });
  },
}));
