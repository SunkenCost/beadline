import type { Locale, ModuleAliasConfig } from "@/types/ipc";

const PALETTE = [
  "#D97757",
  "#6EA1E8",
  "#A78BFA",
  "#46B574",
  "#38BDF8",
  "#E0A84B",
  "#F472B6",
  "#2DD4BF",
] as const;

const BUILTIN_LABELS: Record<string, { zh: string; en: string; color: string }> = {
  kg: { zh: "图谱数据", en: "Knowledge Graph", color: "#D97757" },
  mcp: { zh: "MCP 服务", en: "MCP Server", color: "#6EA1E8" },
  benchmark: { zh: "评测基准", en: "Benchmark", color: "#A78BFA" },
  release: { zh: "工程与发布", en: "Release", color: "#46B574" },
  engine: { zh: "底层引擎", en: "Engine", color: "#D97757" },
  desktop: { zh: "桌面壳层", en: "Desktop", color: "#46B574" },
  ui: { zh: "界面交互", en: "UI", color: "#6EA1E8" },
  agent: { zh: "Agent 协同", en: "Agent", color: "#A78BFA" },
  web: { zh: "前端交互", en: "Web", color: "#38BDF8" },
  eval: { zh: "评测体系", en: "Eval", color: "#E0A84B" },
  docs: { zh: "文档规范", en: "Docs", color: "#94A3B8" },
};

export interface ResolvedModule {
  key: string;
  slug: string;
  label: string;
  color: string;
}

export function resolveModule(
  key: string,
  prefix: string,
  locale: Locale,
  aliases?: Record<string, ModuleAliasConfig>,
): ResolvedModule {
  const effectivePrefix = prefix || "mod:";
  const slug = key.startsWith(effectivePrefix) ? key.slice(effectivePrefix.length) : key;
  const custom = aliases?.[key] ?? aliases?.[slug];
  const builtin = BUILTIN_LABELS[slug.toLowerCase()];

  let hash = 0;
  for (const ch of slug) {
    hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
  }
  const fallbackColor = PALETTE[hash % PALETTE.length] ?? "#D97757";

  return {
    key,
    slug,
    label:
      custom?.label?.trim() || (builtin ? (locale === "zh-CN" ? builtin.zh : builtin.en) : slug),
    color: custom?.color?.trim() || builtin?.color || fallbackColor,
  };
}
