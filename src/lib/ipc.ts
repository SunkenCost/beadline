import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, PlatformInfo } from "@/types/ipc";

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const DEFAULT_SETTINGS: AppSettings = {
  language: "zh-CN",
  theme: {
    preset: "warm-claude",
    appearance: "system",
    accentOverride: null,
    radius: "default",
    density: "comfortable",
    activeCustomThemeId: null,
    customThemes: [],
  },
  bdPath: null,
  useForceWrites: true,
  deepLinkSchemes: ["t3-thread", "vscode", "cursor"],
  window: null,
};

export async function ipcGetSettings(): Promise<AppSettings> {
  if (!isTauriRuntime()) {
    return DEFAULT_SETTINGS;
  }
  return invoke<AppSettings>("get_settings");
}

export async function ipcUpdateSettings(settings: AppSettings): Promise<AppSettings> {
  if (!isTauriRuntime()) {
    return settings;
  }
  return invoke<AppSettings>("update_settings", { settings });
}

export async function ipcGetPlatformInfo(): Promise<PlatformInfo> {
  if (!isTauriRuntime()) {
    return { os: "web", arch: "browser", version: "0.1.0" };
  }
  return invoke<PlatformInfo>("get_platform_info");
}
