import type { AppearanceMode, BeadlineThemeV1, ThemePresetId } from "./ipc";

export const ALLOWED_THEME_TOKEN_KEYS = [
  "bg",
  "sidebar",
  "surface",
  "surface-subtle",
  "surface-hover",
  "surface-active",
  "fg",
  "fg-2",
  "fg-3",
  "border",
  "border-strong",
  "border-subtle",
  "primary",
  "primary-fg",
  "primary-hover",
  "accent",
  "accent-soft",
  "accent-border",
  "success",
  "warning",
  "danger",
  "info",
] as const;

export type AllowedThemeTokenKey = (typeof ALLOWED_THEME_TOKEN_KEYS)[number];

export interface ThemePresetMeta {
  id: ThemePresetId;
  swatchBg: string;
  swatchAccent: string;
}

export const THEME_PRESETS: readonly ThemePresetMeta[] = [
  { id: "warm-clay", swatchBg: "#121211", swatchAccent: "#D97757" },
  { id: "cool-slate", swatchBg: "#0E1013", swatchAccent: "#7A86E8" },
  { id: "pure-mono", swatchBg: "#0B0B0B", swatchAccent: "#E5E5E5" },
] as const;

export type ResolvedAppearance = Exclude<AppearanceMode, "system">;

export type ThemeValidationResult =
  | { ok: true; theme: BeadlineThemeV1 }
  | { ok: false; error: string };
