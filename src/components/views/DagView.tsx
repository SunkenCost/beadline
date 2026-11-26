import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/Button";
import { useT } from "@/i18n";
import { computeDagLayout } from "@/lib/dag-layout";
import { resolveModule } from "@/lib/modules";
import { useSettingsStore } from "@/stores/useSettingsStore";
import { type DagMode, useUiStore } from "@/stores/useUiStore";
import { useWorkspaceStore } from "@/stores/useWorkspaceStore";
import type { BoardLane, DagEdge, IssueDto, ModuleAliasConfig } from "@/types/ipc";

export interface DagViewProps {
  issues: IssueDto[];
  edges: DagEdge[];
  modulePrefix: string;
  moduleAliases?: Record<string, ModuleAliasConfig>;
}

const LANE_COLOR_VAR: Record<BoardLane, string> = {
  in_progress: "var(--accent)",
  ready: "var(--success)",
  blocked: "var(--danger)",
  closed: "var(--fg-3)",
};

export function DagView({ issues, edges, modulePrefix, moduleAliases }: DagViewProps) {
  const t = useT();
  const locale = useSettingsStore((s) => s.settings.language);
  const byId = useWorkspaceStore((s) => s.byId);
  const { dagMode, setDagMode, openDrawer } = useUiStore();

  const vpRef = useRef<HTMLDivElement>(null);
  const [transform, setTransform] = useState({ x: 24, y: 24, scale: 1 });
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const panRef = useRef<{ active: boolean; sx: number; sy: number; ox: number; oy: number }>({
    active: false,
    sx: 0,
    sy: 0,
    ox: 0,
    oy: 0,
  });

  const layout = useMemo(() => computeDagLayout(issues, edges, dagMode), [issues, edges, dagMode]);

  const fitView = useCallback(() => {
    const vp = vpRef.current;
    const vw = vp?.clientWidth ?? 1000;
    const vh = vp?.clientHeight ?? 560;
    const scale = Math.min(
      1.04,
      Math.max(0.4, Math.min((vw - 40) / layout.width, (vh - 40) / layout.height)),
    );
    setTransform({
      scale,
      x: Math.max(16, (vw - layout.width * scale) / 2),
      y: Math.max(16, (vh - layout.height * scale) / 2),
    });
  }, [layout.width, layout.height]);

  useEffect(() => {
    fitView();
  }, [fitView]);

  const highlightedChain = useMemo(() => {
    if (!hoveredId) return null;
    const adj = new Map<string, string[]>();
    const rev = new Map<string, string[]>();
    for (const e of layout.edges) {
      adj.set(e.from, [...(adj.get(e.from) ?? []), e.to]);
      rev.set(e.to, [...(rev.get(e.to) ?? []), e.from]);
    }
    const set = new Set<string>([hoveredId]);
    const walk = (id: string, m: Map<string, string[]>) => {
      for (const nxt of m.get(id) ?? []) {
        if (!set.has(nxt)) {
          set.add(nxt);
          walk(nxt, m);
        }
      }
    };
    walk(hoveredId, adj);
    walk(hoveredId, rev);
    return set;
  }, [hoveredId, layout.edges]);

  const posMap = useMemo(() => new Map(layout.nodes.map((n) => [n.issue.id, n])), [layout.nodes]);

  const modes: readonly { id: DagMode; label: string }[] = [
    { id: "connected", label: t.dag.connected },
    { id: "active", label: t.dag.active },
    { id: "all", label: t.dag.all },
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[var(--border)] px-4 py-2">
        <div className="inline-flex rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface-subtle)] p-0.5">
          {modes.map((m) => (
            <button
              key={m.id}
              type="button"
              onClick={() => setDagMode(m.id)}
              className={`h-6 rounded-[var(--radius-xs)] px-2.5 text-[11.5px] transition-colors cursor-pointer ${
                dagMode === m.id
                  ? "bg-[var(--surface)] font-medium text-[var(--fg)] shadow-[var(--shadow-xs)]"
                  : "text-[var(--fg-2)] hover:text-[var(--fg)]"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>

        <Button size="sm" variant="outline" onClick={fitView}>
          {t.actions.fitView}
        </Button>

        <div className="ml-auto flex items-center gap-3.5 text-[11.5px] text-[var(--fg-3)]">
          {(["in_progress", "ready", "blocked", "closed"] as const).map((lane) => (
            <span key={lane} className="inline-flex items-center gap-1.5">
              <span
                className="h-1.5 w-1.5 rounded-full"
                style={{ backgroundColor: LANE_COLOR_VAR[lane] }}
              />
              <span>{t.lanes[lane]}</span>
            </span>
          ))}
        </div>
      </div>

      <div
        ref={vpRef}
        role="application"
        aria-label={t.nav.dag}
        onMouseDown={(e) => {
          if ((e.target as HTMLElement).closest("[data-dag-node]")) return;
          panRef.current = {
            active: true,
            sx: e.clientX,
            sy: e.clientY,
            ox: transform.x,
            oy: transform.y,
          };
        }}
        onMouseMove={(e) => {
          if (!panRef.current.active) return;
          setTransform((prev) => ({
            ...prev,
            x: panRef.current.ox + (e.clientX - panRef.current.sx),
            y: panRef.current.oy + (e.clientY - panRef.current.sy),
          }));
        }}
        onMouseUp={() => {
          panRef.current.active = false;
        }}
        onMouseLeave={() => {
          panRef.current.active = false;
        }}
        onWheel={(e) => {
          e.preventDefault();
          const vp = vpRef.current;
          if (!vp) return;
          const factor = e.deltaY < 0 ? 1.1 : 0.91;
          const rect = vp.getBoundingClientRect();
          const mx = e.clientX - rect.left;
          const my = e.clientY - rect.top;
          setTransform((prev) => {
            const ns = Math.min(2.2, Math.max(0.32, prev.scale * factor));
            const ratio = ns / prev.scale;
            return {
              scale: ns,
              x: mx - (mx - prev.x) * ratio,
              y: my - (my - prev.y) * ratio,
            };
          });
        }}
        style={{
          backgroundImage: "radial-gradient(var(--border) 1px, transparent 1px)",
          backgroundSize: "20px 20px",
        }}
        className="relative min-h-0 flex-1 overflow-hidden bg-[var(--bg)] cursor-grab active:cursor-grabbing"
      >
        {layout.nodes.length === 0 ? (
          <div className="grid h-full place-items-center text-[12.5px] text-[var(--fg-3)]">
            {t.dag.empty}
          </div>
        ) : (
          <svg className="block h-full w-full" aria-label={t.nav.dag}>
            <defs>
              <marker
                id="arr-active"
                viewBox="0 0 10 10"
                refX="8"
                refY="5"
                markerWidth="5.5"
                markerHeight="5.5"
                orient="auto-start-reverse"
              >
                <path d="M 0 1.5 L 10 5 L 0 8.5 z" fill="var(--danger)" />
              </marker>
              <marker
                id="arr-done"
                viewBox="0 0 10 10"
                refX="8"
                refY="5"
                markerWidth="5.5"
                markerHeight="5.5"
                orient="auto-start-reverse"
              >
                <path d="M 0 1.5 L 10 5 L 0 8.5 z" fill="var(--border-strong)" />
              </marker>
            </defs>

            <g transform={`translate(${transform.x},${transform.y}) scale(${transform.scale})`}>
              {layout.edges.map((e) => {
                const a = posMap.get(e.from);
                const b = posMap.get(e.to);
                if (!a || !b) return null;
                const x1 = a.x + layout.nodeWidth;
                const y1 = a.y + layout.nodeHeight / 2;
                const x2 = b.x - 4;
                const y2 = b.y + layout.nodeHeight / 2;
                const dx = Math.max(30, (x2 - x1) * 0.48);
                const fromClosed = byId[e.from]?.status === "closed";
                const inChain =
                  !highlightedChain || (highlightedChain.has(e.from) && highlightedChain.has(e.to));

                return (
                  <path
                    key={`${e.from}->${e.to}`}
                    d={`M ${x1} ${y1} C ${x1 + dx} ${y1}, ${x2 - dx} ${y2}, ${x2} ${y2}`}
                    fill="none"
                    stroke={fromClosed ? "var(--border-strong)" : "var(--danger)"}
                    strokeWidth={highlightedChain && inChain ? 2.2 : 1.4}
                    strokeDasharray={fromClosed ? undefined : "4 4"}
                    opacity={inChain ? 1 : 0.12}
                    markerEnd={fromClosed ? "url(#arr-done)" : "url(#arr-active)"}
                  />
                );
              })}

              {layout.nodes.map(({ issue: n, x, y }) => {
                const col = LANE_COLOR_VAR[n.lane];
                const inChain = !highlightedChain || highlightedChain.has(n.id);
                const t1 = n.title.length > 20 ? n.title.slice(0, 20) : n.title;
                const t2 =
                  n.title.length > 20
                    ? n.title.length > 40
                      ? `${n.title.slice(20, 39)}…`
                      : n.title.slice(20)
                    : "";
                const firstMod = n.modules[0]
                  ? resolveModule(n.modules[0], modulePrefix, locale, moduleAliases).label
                  : "";
                const foot = [firstMod, n.assignee].filter(Boolean).join(" · ");

                return (
                  // biome-ignore lint/a11y/useSemanticElements: SVG <g> node inside <svg> cannot use HTML <button>
                  <g
                    key={n.id}
                    role="button"
                    tabIndex={0}
                    aria-label={`${n.shortId} ${n.title}`}
                    data-dag-node="true"
                    transform={`translate(${x},${y})`}
                    opacity={inChain ? 1 : 0.18}
                    onMouseEnter={() => setHoveredId(n.id)}
                    onMouseLeave={() => setHoveredId(null)}
                    onClick={(ev) => {
                      ev.stopPropagation();
                      openDrawer(n.id);
                    }}
                    onKeyDown={(ev) => {
                      if (ev.key === "Enter" || ev.key === " ") {
                        ev.preventDefault();
                        openDrawer(n.id);
                      }
                    }}
                    className="cursor-pointer"
                  >
                    <rect
                      width={layout.nodeWidth}
                      height={layout.nodeHeight}
                      rx={7}
                      ry={7}
                      fill="var(--surface)"
                      stroke={
                        hoveredId === n.id || n.lane === "in_progress"
                          ? "var(--accent)"
                          : "var(--border-strong)"
                      }
                      strokeWidth={hoveredId === n.id || n.lane === "in_progress" ? 1.5 : 1}
                    />
                    <circle cx="14" cy="17" r="3" fill={col} />
                    <text
                      x="23"
                      y="20"
                      fontSize="10.5"
                      fill="var(--fg-3)"
                      fontFamily="var(--font-mono)"
                    >
                      {n.shortId} · P{n.priority}
                    </text>
                    <text
                      x={layout.nodeWidth - 12}
                      y="20"
                      textAnchor="end"
                      fontSize="10.5"
                      fill={col}
                    >
                      {t.lanes[n.lane]}
                    </text>
                    <text x="12" y="39" fontSize="12" fontWeight="500" fill="var(--fg)">
                      {t1}
                    </text>
                    {t2 && (
                      <text x="12" y="55" fontSize="12" fontWeight="500" fill="var(--fg)">
                        {t2}
                      </text>
                    )}
                    {foot && (
                      <text
                        x="12"
                        y="68"
                        fontSize="10"
                        fill="var(--fg-3)"
                        fontFamily="var(--font-mono)"
                      >
                        {foot}
                      </text>
                    )}
                  </g>
                );
              })}
            </g>
          </svg>
        )}

        <div className="absolute right-3.5 bottom-3.5 flex items-center gap-0.5 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--surface)] p-0.5 shadow-[var(--shadow-xs)]">
          <button
            type="button"
            onClick={() => setTransform((p) => ({ ...p, scale: Math.max(0.32, p.scale * 0.85) }))}
            className="h-6 min-w-[24px] rounded-[var(--radius-xs)] px-1.5 font-mono-tabular text-[11.5px] text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
          >
            −
          </button>
          <button
            type="button"
            onClick={fitView}
            className="h-6 rounded-[var(--radius-xs)] px-1.5 font-mono-tabular text-[11px] text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
          >
            {Math.round(transform.scale * 100)}%
          </button>
          <button
            type="button"
            onClick={() => setTransform((p) => ({ ...p, scale: Math.min(2.2, p.scale * 1.15) }))}
            className="h-6 min-w-[24px] rounded-[var(--radius-xs)] px-1.5 font-mono-tabular text-[11.5px] text-[var(--fg-2)] hover:bg-[var(--surface-hover)] hover:text-[var(--fg)] cursor-pointer"
          >
            +
          </button>
        </div>
      </div>
    </div>
  );
}
