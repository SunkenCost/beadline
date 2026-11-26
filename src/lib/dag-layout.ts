import type { DagEdge, IssueDto } from "@/types/ipc";

export interface DagNodeLayout {
  issue: IssueDto;
  x: number;
  y: number;
  layer: number;
}

export interface DagLayoutResult {
  nodes: DagNodeLayout[];
  edges: DagEdge[];
  width: number;
  height: number;
  nodeWidth: number;
  nodeHeight: number;
}

export const DAG_NODE_WIDTH = 248;
export const DAG_NODE_HEIGHT = 76;
const GAP_X = 76;
const GAP_Y = 16;

export function computeDagLayout(
  issues: IssueDto[],
  rawEdges: DagEdge[],
  mode: "connected" | "active" | "all",
): DagLayoutResult {
  const validIds = new Set(issues.map((i) => i.id));
  const scopedEdges = rawEdges.filter((e) => validIds.has(e.from) && validIds.has(e.to));

  const nodeSet = new Set<string>();
  if (mode === "all") {
    for (const it of issues) nodeSet.add(it.id);
  } else if (mode === "active") {
    const activeIds = new Set(issues.filter((x) => x.lane !== "closed").map((x) => x.id));
    for (const e of scopedEdges) {
      if (activeIds.has(e.from) || activeIds.has(e.to)) {
        nodeSet.add(e.from);
        nodeSet.add(e.to);
      }
    }
    for (const it of issues) {
      if (it.lane !== "closed" && it.issueType !== "epic") nodeSet.add(it.id);
    }
  } else {
    for (const e of scopedEdges) {
      nodeSet.add(e.from);
      nodeSet.add(e.to);
    }
  }

  const nodes = issues.filter((i) => nodeSet.has(i.id));
  const edges = scopedEdges.filter((e) => nodeSet.has(e.from) && nodeSet.has(e.to));

  if (nodes.length === 0) {
    return {
      nodes: [],
      edges: [],
      width: 600,
      height: 360,
      nodeWidth: DAG_NODE_WIDTH,
      nodeHeight: DAG_NODE_HEIGHT,
    };
  }

  const indeg = new Map<string, number>();
  const adj = new Map<string, string[]>();
  const rev = new Map<string, string[]>();
  for (const n of nodes) {
    indeg.set(n.id, 0);
    adj.set(n.id, []);
    rev.set(n.id, []);
  }
  for (const e of edges) {
    adj.get(e.from)?.push(e.to);
    rev.get(e.to)?.push(e.from);
    indeg.set(e.to, (indeg.get(e.to) ?? 0) + 1);
  }

  const depth = new Map<string, number>();
  for (const n of nodes) depth.set(n.id, 0);
  const queue = nodes.filter((n) => (indeg.get(n.id) ?? 0) === 0).map((n) => n.id);

  while (queue.length > 0) {
    const u = queue.shift();
    if (!u) break;
    const d = depth.get(u) ?? 0;
    for (const v of adj.get(u) ?? []) {
      depth.set(v, Math.max(depth.get(v) ?? 0, d + 1));
      const nextIn = (indeg.get(v) ?? 1) - 1;
      indeg.set(v, nextIn);
      if (nextIn === 0) queue.push(v);
    }
  }

  const layers = new Map<number, IssueDto[]>();
  for (const n of nodes) {
    const l = depth.get(n.id) ?? 0;
    const bucket = layers.get(l);
    if (bucket) bucket.push(n);
    else layers.set(l, [n]);
  }

  const sortedLayerKeys = [...layers.keys()].sort((a, b) => a - b);
  const posY = new Map<string, number>();
  const laidOut: DagNodeLayout[] = [];
  let maxX = 0;
  let maxY = 0;

  for (const lk of sortedLayerKeys) {
    const arr = layers.get(lk) ?? [];
    arr.sort((a, b) => {
      const pa = (rev.get(a.id) ?? []).map((p) => posY.get(p) ?? 0);
      const pb = (rev.get(b.id) ?? []).map((p) => posY.get(p) ?? 0);
      const ya = pa.length ? pa.reduce((s, v) => s + v, 0) / pa.length : a.priority * 100;
      const yb = pb.length ? pb.reduce((s, v) => s + v, 0) / pb.length : b.priority * 100;
      return ya - yb || a.priority - b.priority || a.id.localeCompare(b.id);
    });

    const totalH = arr.length * DAG_NODE_HEIGHT + Math.max(0, arr.length - 1) * GAP_Y;
    const startY = Math.max(28, (540 - totalH) / 2);

    arr.forEach((issue, idx) => {
      const x = 28 + lk * (DAG_NODE_WIDTH + GAP_X);
      const y = startY + idx * (DAG_NODE_HEIGHT + GAP_Y);
      posY.set(issue.id, y);
      laidOut.push({ issue, x, y, layer: lk });
      maxX = Math.max(maxX, x + DAG_NODE_WIDTH + 40);
      maxY = Math.max(maxY, y + DAG_NODE_HEIGHT + 40);
    });
  }

  return {
    nodes: laidOut,
    edges,
    width: Math.max(600, maxX),
    height: Math.max(360, maxY),
    nodeWidth: DAG_NODE_WIDTH,
    nodeHeight: DAG_NODE_HEIGHT,
  };
}
