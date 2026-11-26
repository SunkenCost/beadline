import { resolveModule } from "@/lib/modules";
import type { BoardLane, Locale, ModuleAliasConfig } from "@/types/ipc";

export function StatusRing({ lane, size = 13 }: { lane: BoardLane; size?: number }) {
  if (lane === "in_progress") {
    return (
      <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <circle cx="8" cy="8" r="6" stroke="var(--accent)" strokeWidth="1.5" />
        <path d="M8 4 A4 4 0 0 1 12 8 L8 8 Z" fill="var(--accent)" />
      </svg>
    );
  }
  if (lane === "ready") {
    return (
      <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <circle
          cx="8"
          cy="8"
          r="6"
          stroke="var(--success)"
          strokeWidth="1.5"
          strokeDasharray="2.5 2"
        />
      </svg>
    );
  }
  if (lane === "blocked") {
    return (
      <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <circle cx="8" cy="8" r="6" stroke="var(--danger)" strokeWidth="1.5" />
        <line x1="4.5" y1="11.5" x2="11.5" y2="4.5" stroke="var(--danger)" strokeWidth="1.4" />
      </svg>
    );
  }
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="6" stroke="var(--fg-3)" strokeWidth="1.4" />
      <polyline
        points="5.2 8.2 7.2 10.2 11 6.2"
        stroke="var(--fg-3)"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function PriorityGlyph({ priority }: { priority: number }) {
  const n = Math.min(4, Math.max(0, priority));
  if (n === 0) {
    return (
      <span className="inline-flex shrink-0 items-center" title="P0">
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect
            x="2"
            y="2"
            width="12"
            height="12"
            rx="3"
            fill="var(--danger)"
            fillOpacity="0.16"
            stroke="var(--danger)"
            strokeWidth="1.2"
          />
          <line
            x1="8"
            y1="5"
            x2="8"
            y2="8.6"
            stroke="var(--danger)"
            strokeWidth="1.6"
            strokeLinecap="round"
          />
          <circle cx="8" cy="11.2" r="0.9" fill="var(--danger)" />
        </svg>
      </span>
    );
  }
  if (n === 1) {
    return (
      <span className="inline-flex shrink-0 items-center" title="P1">
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect x="2.5" y="9.5" width="2.2" height="4" rx="0.8" fill="var(--accent)" />
          <rect x="6.9" y="6.5" width="2.2" height="7" rx="0.8" fill="var(--accent)" />
          <rect x="11.3" y="3.5" width="2.2" height="10" rx="0.8" fill="var(--accent)" />
        </svg>
      </span>
    );
  }
  if (n === 2) {
    return (
      <span className="inline-flex shrink-0 items-center" title="P2">
        <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect x="2.5" y="9.5" width="2.2" height="4" rx="0.8" fill="var(--fg-2)" />
          <rect x="6.9" y="6.5" width="2.2" height="7" rx="0.8" fill="var(--fg-2)" />
          <rect x="11.3" y="3.5" width="2.2" height="10" rx="0.8" fill="var(--border-strong)" />
        </svg>
      </span>
    );
  }
  return (
    <span className="inline-flex shrink-0 items-center" title={`P${n}`}>
      <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
        <rect x="2.5" y="9.5" width="2.2" height="4" rx="0.8" fill="var(--fg-3)" />
        <rect x="6.9" y="6.5" width="2.2" height="7" rx="0.8" fill="var(--border-strong)" />
        <rect x="11.3" y="3.5" width="2.2" height="10" rx="0.8" fill="var(--border-strong)" />
      </svg>
    </span>
  );
}

export function AssigneeDot({ name }: { name: string }) {
  if (!name) return null;
  const dotColor = name.toLowerCase().includes("gemini")
    ? "#7C7FF0"
    : name.toLowerCase().includes("gpt")
      ? "#46B574"
      : name.toLowerCase().includes("claude")
        ? "#D97757"
        : "var(--fg-3)";
  return (
    <span
      className="inline-flex max-w-[116px] items-center gap-1 truncate font-mono-tabular text-[10.5px] text-[var(--fg-3)]"
      title={name}
    >
      <span className="h-1.5 w-1.5 shrink-0 rounded-full" style={{ backgroundColor: dotColor }} />
      <span className="truncate">{name}</span>
    </span>
  );
}

export function ModuleInlineList({
  modules,
  prefix,
  locale,
  aliases,
  limit = 2,
}: {
  modules: string[];
  prefix: string;
  locale: Locale;
  aliases?: Record<string, ModuleAliasConfig>;
  limit?: number;
}) {
  if (!modules.length) return null;
  return (
    <>
      {modules.slice(0, limit).map((k) => {
        const m = resolveModule(k, prefix, locale, aliases);
        return (
          <span
            key={k}
            className="inline-flex items-center gap-1 text-[11px] text-[var(--fg-2)]"
            title={k}
          >
            <span
              className="h-1.5 w-1.5 shrink-0 rounded-full"
              style={{ backgroundColor: m.color }}
            />
            <span>{m.label}</span>
          </span>
        );
      })}
    </>
  );
}
