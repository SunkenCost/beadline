import { useSettingsStore } from "@/stores/useSettingsStore";
import type { Locale } from "@/types/ipc";
import { en } from "./en";
import { zhCN } from "./zh-CN";

const CATALOGS = {
  "zh-CN": zhCN,
  en,
} as const;

export function getCatalog(locale: Locale) {
  return CATALOGS[locale] ?? zhCN;
}

export function useT() {
  const locale = useSettingsStore((s) => s.settings.language);
  return getCatalog(locale);
}
