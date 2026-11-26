export type Locale = "zh-CN" | "en";

export type ThemePresetId = "warm-claude" | "linear-slate" | "pure-mono";

export type AppearanceMode = "light" | "dark" | "system";

export type RadiusScale = "sharp" | "default" | "soft";

export type DensityMode = "compact" | "comfortable";

export interface BeadlineThemeV1 {
  schemaVersion: 1;
  id: string;
  name: string;
  appearance: AppearanceMode;
  tokens: Record<string, string>;
}

export interface ThemeSettings {
  preset: ThemePresetId;
  appearance: AppearanceMode;
  accentOverride: string | null;
  radius: RadiusScale;
  density: DensityMode;
  activeCustomThemeId: string | null;
  customThemes: BeadlineThemeV1[];
}

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
  maximized: boolean;
}

export interface AppSettings {
  language: Locale;
  theme: ThemeSettings;
  bdPath: string | null;
  useForceWrites: boolean;
  deepLinkSchemes: string[];
  window: WindowGeometry | null;
}

export interface PlatformInfo {
  os: string;
  arch: string;
  version: string;
}

export interface AppErrorDto {
  code: string;
  message: string;
  detail?: string | null;
}
