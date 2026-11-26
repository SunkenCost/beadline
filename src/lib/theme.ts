import type { AppearanceMode, BeadlineThemeV1, ThemeSettings } from "@/types/ipc";
import {
  ALLOWED_THEME_TOKEN_KEYS,
  type ResolvedAppearance,
  type ThemeValidationResult,
} from "@/types/theme";

const ALLOWED_KEYS = new Set<string>(ALLOWED_THEME_TOKEN_KEYS);

// Strict color validation: accepts #RGB/#RRGGBB/#RRGGBBAA, rgb()/rgba(), hsl()/hsla(), oklch()
const SAFE_COLOR_RE = /^(#[0-9a-fA-F]{3,8}|(rgb|rgba|hsl|hsla|oklch)\(\s*[0-9.,%\s/+-]+\s*\))$/;

export function isSafeCssColor(value: string): boolean {
  const trimmed = value.trim();
  if (trimmed.length === 0 || trimmed.length > 64) {
    return false;
  }
  if (/[;{}<>()"'\\]/.test(trimmed.replace(/[()]/g, ""))) {
    return false;
  }
  return SAFE_COLOR_RE.test(trimmed);
}

export function resolveAppearance(
  mode: AppearanceMode,
  systemPrefersDark: boolean,
): ResolvedAppearance {
  if (mode === "system") {
    return systemPrefersDark ? "dark" : "light";
  }
  return mode;
}

export function applyThemeToDocument(
  theme: ThemeSettings,
  systemPrefersDark: boolean,
): ResolvedAppearance {
  const root = document.documentElement;
  const activeCustom = theme.activeCustomThemeId
    ? (theme.customThemes.find((t) => t.id === theme.activeCustomThemeId) ?? null)
    : null;

  const effectiveMode = activeCustom ? activeCustom.appearance : theme.appearance;
  const resolved = resolveAppearance(effectiveMode, systemPrefersDark);

  root.dataset.preset = theme.preset;
  root.dataset.theme = resolved;
  root.dataset.radius = theme.radius;
  root.dataset.density = theme.density;

  // Clear any previously applied custom overrides first
  for (const key of ALLOWED_THEME_TOKEN_KEYS) {
    root.style.removeProperty(`--${key}`);
  }

  if (activeCustom) {
    for (const [key, value] of Object.entries(activeCustom.tokens)) {
      if (ALLOWED_KEYS.has(key) && isSafeCssColor(value)) {
        root.style.setProperty(`--${key}`, value.trim());
      }
    }
  }

  if (theme.accentOverride && isSafeCssColor(theme.accentOverride)) {
    root.style.setProperty("--accent", theme.accentOverride.trim());
  }

  try {
    localStorage.setItem("beadline:preset", theme.preset);
    localStorage.setItem("beadline:appearance", theme.appearance);
    localStorage.setItem("beadline:radius", theme.radius);
    localStorage.setItem("beadline:density", theme.density);
  } catch {
    // Ignore storage quota / privacy errors
  }

  return resolved;
}

export function parseThemeJson(raw: string): ThemeValidationResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: "Invalid JSON syntax" };
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { ok: false, error: "Theme must be a JSON object" };
  }

  const obj = parsed as Record<string, unknown>;
  if (obj.schemaVersion !== 1) {
    return { ok: false, error: "Unsupported schemaVersion (expected 1)" };
  }

  if (typeof obj.id !== "string" || !/^[a-z0-9][a-z0-9-_]{1,48}$/.test(obj.id)) {
    return { ok: false, error: "Invalid theme id (lowercase alphanumeric, hyphen, underscore)" };
  }

  if (typeof obj.name !== "string" || obj.name.trim().length === 0 || obj.name.length > 64) {
    return { ok: false, error: "Theme name must be 1–64 characters" };
  }

  if (obj.appearance !== "light" && obj.appearance !== "dark" && obj.appearance !== "system") {
    return { ok: false, error: "appearance must be light, dark, or system" };
  }

  if (!obj.tokens || typeof obj.tokens !== "object" || Array.isArray(obj.tokens)) {
    return { ok: false, error: "tokens must be a key-value map" };
  }

  const sanitizedTokens: Record<string, string> = {};
  for (const [k, v] of Object.entries(obj.tokens as Record<string, unknown>)) {
    if (!ALLOWED_KEYS.has(k)) {
      return { ok: false, error: `Unknown theme token: ${k}` };
    }
    if (typeof v !== "string" || !isSafeCssColor(v)) {
      return { ok: false, error: `Invalid color value for token "${k}"` };
    }
    sanitizedTokens[k] = v.trim();
  }

  const theme: BeadlineThemeV1 = {
    schemaVersion: 1,
    id: obj.id,
    name: obj.name.trim(),
    appearance: obj.appearance,
    tokens: sanitizedTokens,
  };

  return { ok: true, theme };
}
