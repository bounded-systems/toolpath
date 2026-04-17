// DAG renderer adapted from site/js/visualizer.js.
// Takes a parsed Toolpath Document + a target SVG element.

import * as d3 from "d3";
// dagre-d3-es is a module-ESM fork of dagre-d3; API identical but split into
// named exports. Types are incomplete, so the render factory is cast through
// `unknown` to a callable signature.
import { graphlib, render as dagreRenderRaw } from "dagre-d3-es";
const dagreRender = dagreRenderRaw as unknown as () => (
  group: unknown,
  graph: unknown,
) => void;

import type { ActorDef, Document, StepRef } from "./types";

const COLORS = {
  human: { fill: "#b5652b18", stroke: "#b5652b" },
  agent: { fill: "#b5652b30", stroke: "#b5652b" },
  tool: { fill: "#8a807815", stroke: "#8a8078" },
  ci: { fill: "#8a807815", stroke: "#8a8078" },
  dead: { fill: "#c4403018", stroke: "#c44030" },
  base: { fill: "#ece5db", stroke: "#8a8078" },
};
const EDGE_ACTIVE = { stroke: "#2d2a26", width: 2 };
const EDGE_INACTIVE = { stroke: "#8a8078", width: 1 };
const EDGE_BASE = { stroke: "#b5652b", width: 1.5 };

function actorType(a: string): keyof typeof COLORS | "tool" {
  const i = a.indexOf(":");
  const t = i < 0 ? a : a.substring(0, i);
  return (t in COLORS ? (t as keyof typeof COLORS) : "tool");
}
function actorName(a: string): string {
  const i = a.indexOf(":");
  return i < 0 ? a : a.substring(i + 1);
}
function actorDisplay(a: string, defs: Record<string, ActorDef> | null): string {
  const def = defs?.[a];
  return def?.name ?? actorName(a);
}
function truncate(s: string | undefined, n: number): string {
  return s && s.length > n ? `${s.substring(0, n)}…` : s ?? "";
}

function ancestors(steps: StepRef[], headId: string): Record<string, true> {
  const map: Record<string, StepRef> = {};
  for (const s of steps) map[s.step.id] = s;
  const out: Record<string, true> = {};
  const stack = [headId];
  while (stack.length) {
    const id = stack.pop()!;
    if (out[id]) continue;
    out[id] = true;
    const s = map[id];
    if (s?.step.parents) for (const p of s.step.parents) stack.push(p);
  }
  return out;
}

interface Cluster {
  pathInfo: { id: string; head?: string } | null;
  steps: StepRef[];
  headId: string | null;
  base: { uri: string; ref?: string } | null;
  actors: Record<string, ActorDef> | null;
  isRef?: boolean;
}

function normalizeClusters(doc: Document): Cluster[] {
  if ("Step" in doc) {
    return [
      {
        pathInfo: null,
        steps: [doc.Step],
        headId: null,
        base: null,
        actors: doc.Step.meta?.actors ?? null,
      },
    ];
  }
  if ("Path" in doc) {
    const p = doc.Path;
    return [
      {
        pathInfo: p.path,
        steps: p.steps,
        headId: p.path.head,
        base: p.path.base ?? null,
        actors: p.meta?.actors ?? null,
      },
    ];
  }
  if ("Graph" in doc) {
    const g = doc.Graph;
    const gActors = g.meta?.actors ?? null;
    return g.paths.map((e) => {
      if ("$ref" in e) {
        return {
          pathInfo: { id: e.$ref },
          steps: [],
          headId: null,
          base: null,
          isRef: true,
          actors: gActors,
        };
      }
      return {
        pathInfo: e.path,
        steps: e.steps,
        headId: e.path.head,
        base: e.path.base ?? null,
        actors: (e as { meta?: { actors?: Record<string, ActorDef> } }).meta?.actors ?? gActors,
      };
    });
  }
  return [];
}

export interface RenderOpts {
  showDead: boolean;
  showTs: boolean;
  showFiles: boolean;
  onStepClick?: (step: StepRef, actors: Record<string, ActorDef> | null) => void;
}

export function render(
  doc: Document,
  svgEl: SVGSVGElement,
  opts: RenderOpts,
): { fit: () => void } | null {
  const clusters = normalizeClusters(doc);
  if (!clusters.length) return null;

  const graph = new graphlib.Graph({ compound: true, multigraph: false })
    .setGraph({ rankdir: "TB", nodesep: 60, ranksep: 50, marginx: 30, marginy: 30 })
    .setDefaultEdgeLabel(() => ({}));

  clusters.forEach((cluster, ci) => {
    const prefix = clusters.length > 1 ? `c${ci}/` : "";
    const anc = cluster.headId ? ancestors(cluster.steps, cluster.headId) : null;

    if (clusters.length > 1) {
      graph.setNode(`cluster_${ci}`, {
        label: cluster.pathInfo?.id ?? `cluster-${ci}`,
        clusterLabelPos: "top",
        style: "fill: transparent; stroke: #b5652b26; stroke-dasharray: 4,3;",
      });
    }

    if (cluster.base) {
      const baseId = `${prefix}__BASE__`;
      graph.setNode(baseId, {
        label: "BASE",
        shape: "ellipse",
        style: `fill: ${COLORS.base.fill}; stroke: ${COLORS.base.stroke}; stroke-width: 2px;`,
        labelStyle: "font-family: 'IBM Plex Mono', monospace; font-size: 10px; font-weight: 600;",
      });
      if (clusters.length > 1) graph.setParent(baseId, `cluster_${ci}`);
    }

    if (cluster.isRef) {
      const refId = `${prefix}${cluster.pathInfo!.id}`;
      graph.setNode(refId, {
        label: `$ref: ${cluster.pathInfo!.id}`,
        shape: "rect",
        style: "fill: #8a807815; stroke: #8a8078; stroke-dasharray: 4,3; stroke-width: 1px;",
        labelStyle: "font-family: 'IBM Plex Mono', monospace; font-size: 10px; font-style: italic;",
      });
      return;
    }

    const roots: string[] = [];
    for (const s of cluster.steps) {
      const sid = s.step.id;
      const nodeId = `${prefix}${sid}`;
      const isDead = anc && !anc[sid];
      const isHead = sid === cluster.headId;
      if (isDead && !opts.showDead) continue;
      if (!s.step.parents || !s.step.parents.length) roots.push(nodeId);

      const t = actorType(s.step.actor);
      const colors = COLORS[t];
      const lines = [sid, actorDisplay(s.step.actor, cluster.actors)];
      if (s.meta?.intent) lines.push(truncate(s.meta.intent, 30));
      if (opts.showTs && s.step.timestamp) lines.push(s.step.timestamp.substring(11, 19));
      if (opts.showFiles && s.change) for (const f of Object.keys(s.change)) lines.push(truncate(f, 28));

      const fill = isDead ? COLORS.dead.fill : colors.fill;
      const stroke = isDead ? COLORS.dead.stroke : colors.stroke;
      graph.setNode(nodeId, {
        label: lines.join("\n"),
        shape: "rect",
        style: `fill: ${fill}; stroke: ${stroke}; stroke-width: ${isHead ? "3px" : "1.5px"}; stroke-dasharray: ${isDead || t === "ci" ? "4,3" : "none"};`,
        labelStyle: `font-family: 'IBM Plex Mono', monospace; font-size: 10px; ${isHead ? "font-weight: bold;" : ""}`,
        _step: s,
        _clusterIndex: ci,
        _isDead: isDead,
        _isHead: isHead,
      });
      if (clusters.length > 1) graph.setParent(nodeId, `cluster_${ci}`);
    }

    for (const s of cluster.steps) {
      const sid = s.step.id;
      const targetId = `${prefix}${sid}`;
      const isDead = anc && !anc[sid];
      if (isDead && !opts.showDead) continue;
      if (!s.step.parents) continue;
      for (const pid of s.step.parents) {
        const srcId = `${prefix}${pid}`;
        if (!graph.node(srcId)) continue;
        if (!opts.showDead && anc && !anc[pid]) continue;
        const bothActive = anc && anc[sid] && anc[pid];
        const style = bothActive ? EDGE_ACTIVE : EDGE_INACTIVE;
        const dash = bothActive ? "" : "4,3";
        graph.setEdge(srcId, targetId, {
          style: `stroke: ${style.stroke}; stroke-width: ${style.width}px;${dash ? ` stroke-dasharray: ${dash};` : ""}`,
          arrowheadStyle: `fill: ${style.stroke}`,
          curve: d3.curveBasis,
        });
      }
    }

    if (cluster.base) {
      const baseNodeId = `${prefix}__BASE__`;
      for (const rid of roots) {
        if (graph.node(rid)) {
          graph.setEdge(baseNodeId, rid, {
            style: `stroke: ${EDGE_BASE.stroke}; stroke-width: ${EDGE_BASE.width}px;`,
            arrowheadStyle: `fill: ${EDGE_BASE.stroke}`,
            curve: d3.curveBasis,
          });
        }
      }
    }
  });

  const svg = d3.select<SVGSVGElement, unknown>(svgEl);
  svg.selectAll("*").remove();
  const group = svg.append("g");
  dagreRender()(group, graph);

  const zoom = d3
    .zoom<SVGSVGElement, unknown>()
    .scaleExtent([0.1, 4])
    .on("zoom", (ev) => group.attr("transform", ev.transform));
  svg.call(zoom);

  function fit() {
    const gNode = group.node();
    if (!gNode) return;
    const bounds = (gNode as SVGGraphicsElement).getBBox();
    if (!bounds.width || !bounds.height) return;
    const parent = svgEl.parentElement;
    if (!parent) return;
    const sx = parent.clientWidth / (bounds.width + 60);
    const sy = parent.clientHeight / (bounds.height + 60);
    const scale = Math.min(sx, sy, 1.5);
    const tx = (parent.clientWidth - bounds.width * scale) / 2 - bounds.x * scale;
    const ty = (parent.clientHeight - bounds.height * scale) / 2 - bounds.y * scale;
    svg
      .transition()
      .duration(300)
      .call(zoom.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
  }
  fit();

  if (opts.onStepClick) {
    svg.selectAll<SVGGElement, string>("g.node").on("click", function () {
      // dagre-d3 stashes node id on __data__
      const id = (this as SVGGElement & { __data__: string }).__data__;
      const data = graph.node(id) as {
        _step?: StepRef;
        _clusterIndex?: number;
      } | undefined;
      if (data?._step && typeof data._clusterIndex === "number") {
        const cluster = clusters[data._clusterIndex];
        opts.onStepClick!(data._step, cluster?.actors ?? null);
      }
    });
  }

  return { fit };
}
