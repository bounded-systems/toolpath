// Path graph renderer.
//
// Renders a Toolpath Document as a dagre-laid-out DAG of HTML "cards"
// connected by SVG edges. Dead-end subtrees are hidden behind their
// HEAD-path sibling until the user clicks the card's "expand" chip —
// clicking the chip fires `onToggleBranch(nodeId)`, which flips a key in
// `expandedBranches` and causes the graph to re-layout including those
// nodes.
//
// Usage: `render(doc, containerEl, opts)`. Call again on any state change
// (selected / expanded / toggles) — the function rebuilds the DOM under
// `containerEl` from scratch.

import * as dagre from "@dagrejs/dagre";
import type { ActorDef, Document, StepRef } from "./types";

export interface RenderOpts {
  selectedStepId: string | null;
  expandedBranches: Record<string, true>;
  showTs: boolean;
  showFiles: boolean;
  onSelectStep: (step: StepRef, actors: Record<string, ActorDef> | null) => void;
  onToggleBranch: (stepId: string) => void;
}

// ─── Document normalization ──────────────────────────────────────────────

interface Normalized {
  steps: StepRef[];
  head: string | null;
  actors: Record<string, ActorDef> | null;
  stepMap: Map<string, StepRef>;
  childrenMap: Map<string, string[]>;
  headSet: Set<string>;
}

function normalize(doc: Document): Normalized {
  let steps: StepRef[] = [];
  let head: string | null = null;
  let actors: Record<string, ActorDef> | null = null;

  if ("Step" in doc) {
    steps = [doc.Step];
    actors = doc.Step.meta?.actors ?? null;
  } else if ("Path" in doc) {
    steps = doc.Path.steps;
    head = doc.Path.path.head;
    actors = doc.Path.meta?.actors ?? null;
  } else if ("Graph" in doc) {
    actors = doc.Graph.meta?.actors ?? null;
    for (const p of doc.Graph.paths) {
      if ("$ref" in p) continue;
      if (head == null) head = p.path.head;
      for (const s of p.steps) steps.push(s);
    }
  }

  const stepMap = new Map<string, StepRef>(steps.map((s) => [s.step.id, s]));
  const childrenMap = new Map<string, string[]>();
  for (const s of steps) {
    for (const pid of s.step.parents || []) {
      const list = childrenMap.get(pid);
      if (list) list.push(s.step.id);
      else childrenMap.set(pid, [s.step.id]);
    }
  }

  const headSet = new Set<string>();
  if (head && stepMap.has(head)) {
    const stack: string[] = [head];
    while (stack.length) {
      const id = stack.pop()!;
      if (headSet.has(id)) continue;
      headSet.add(id);
      const s = stepMap.get(id);
      if (s?.step.parents) for (const p of s.step.parents) stack.push(p);
    }
  }
  return { steps, head, actors, stepMap, childrenMap, headSet };
}

// ─── Visibility + helpers ────────────────────────────────────────────────

function actorType(a: string): string {
  const i = a.indexOf(":");
  return i < 0 ? a : a.slice(0, i);
}
function actorName(
  a: string,
  actors: Record<string, ActorDef> | null,
): string {
  const def = actors?.[a];
  if (def?.name) return def.name;
  const i = a.indexOf(":");
  return i < 0 ? a : a.slice(i + 1);
}
function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
function cssSafeId(id: string): string {
  return id.replace(/[^a-zA-Z0-9_-]/g, "_");
}

/** Walk dead-node → parent chain until hitting a HEAD-path ancestor. */
function findHeadAncestor(
  id: string,
  stepMap: Map<string, StepRef>,
  headSet: Set<string>,
): string | null {
  if (headSet.has(id)) return null;
  let current: string | undefined = id;
  while (current) {
    const s = stepMap.get(current);
    const parents = s?.step.parents;
    if (!parents?.length) return null;
    const p: string = parents[0];
    if (headSet.has(p)) return p;
    current = p;
  }
  return null;
}

function deadChildrenCount(
  id: string,
  headSet: Set<string>,
  childrenMap: Map<string, string[]>,
): number {
  if (!headSet.has(id)) return 0;
  const kids = childrenMap.get(id) ?? [];
  let n = 0;
  for (const k of kids) if (!headSet.has(k)) n++;
  return n;
}

function isStepVisible(
  id: string,
  headSet: Set<string>,
  stepMap: Map<string, StepRef>,
  expandedBranches: Record<string, true>,
): boolean {
  if (headSet.has(id)) return true;
  const anc = findHeadAncestor(id, stepMap, headSet);
  return !!(anc && expandedBranches[anc]);
}

// ─── Rendering ────────────────────────────────────────────────────────────

function renderCard(
  s: StepRef,
  flags: {
    isHead: boolean;
    isDead: boolean;
    isFocused: boolean;
    deadKids: number;
    isExpanded: boolean;
    actors: Record<string, ActorDef> | null;
    showTs: boolean;
    showFiles: boolean;
  },
): string {
  const id = s.step.id;
  const atype = actorType(s.step.actor);
  const changeKeys = s.change ? Object.keys(s.change) : [];
  const hasHidden = flags.deadKids > 0 && !flags.isExpanded;

  const classes = [
    "pg-card",
    `pg-card--role-${atype}`,
    flags.isHead ? "pg-card--head" : "",
    flags.isDead ? "pg-card--dead" : "",
    flags.isFocused ? "pg-card--focused" : "",
    hasHidden ? "pg-card--has-hidden" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const chips: string[] = [];
  if (flags.isHead)
    chips.push(`<span class="pg-card__chip pg-card__chip--head">HEAD</span>`);
  if (flags.isDead)
    chips.push(`<span class="pg-card__chip pg-card__chip--dead">dead</span>`);

  const toggle =
    flags.deadKids > 0
      ? `<button class="pg-card__toggle${flags.isExpanded ? " pg-card__toggle--on" : ""}" data-toggle-branch="${esc(id)}">${flags.isExpanded ? "collapse" : "expand"}</button>`
      : "";

  const intent = s.meta?.intent ? esc(s.meta.intent) : "";
  const ts =
    flags.showTs && s.step.timestamp
      ? `<div class="pg-card__ts">${esc(s.step.timestamp.replace("T", " ").replace("Z", " UTC"))}</div>`
      : "";
  const files =
    flags.showFiles && changeKeys.length
      ? `<div class="pg-card__files">${changeKeys
          .map((k) => `<span>${esc(k)}</span>`)
          .join(" · ")}</div>`
      : "";

  return `
    <div class="${classes}" id="pg-card-${cssSafeId(id)}" data-step-id="${esc(id)}">
      <div class="pg-card__head">
        <span class="pg-card__id">${esc(id)}</span>
        <div class="pg-card__chips">${chips.join("")}</div>
      </div>
      ${intent ? `<div class="pg-card__intent">${intent}</div>` : ""}
      <div class="pg-card__actor">${esc(actorName(s.step.actor, flags.actors))}</div>
      ${ts}
      ${files}
      ${toggle ? `<div class="pg-card__footer">${toggle}</div>` : ""}
    </div>`;
}

function pointsToPath(pts: { x: number; y: number }[]): string {
  if (pts.length === 0) return "";
  if (pts.length === 1) return `M ${pts[0].x} ${pts[0].y}`;
  let d = `M ${pts[0].x} ${pts[0].y}`;
  for (let i = 1; i < pts.length; i++) d += ` L ${pts[i].x} ${pts[i].y}`;
  return d;
}

const NS = "http://www.w3.org/2000/svg";

export function render(
  doc: Document,
  container: HTMLElement,
  opts: RenderOpts,
): void {
  const { steps, head, actors, stepMap, childrenMap, headSet } = normalize(doc);

  // Build container DOM
  container.innerHTML = "";
  const graphEl = document.createElement("div");
  graphEl.className = "path-graph";
  const svgEl = document.createElementNS(NS, "svg");
  svgEl.setAttribute("class", "path-graph__edges");
  // Arrow markers live in <defs>
  const defs = document.createElementNS(NS, "defs");
  defs.innerHTML = `
    <marker id="pg-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#8a8078" />
    </marker>
    <marker id="pg-arrow-dead" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#c44030" fill-opacity="0.8" />
    </marker>`;
  svgEl.appendChild(defs);
  const nodesEl = document.createElement("div");
  nodesEl.className = "path-graph__nodes";
  graphEl.appendChild(svgEl);
  graphEl.appendChild(nodesEl);
  container.appendChild(graphEl);

  if (steps.length === 0) {
    nodesEl.innerHTML = `<div class="path-graph__empty">This document has no steps to visualize.</div>`;
    return;
  }

  // Visible step set
  const visible = steps.filter((s) =>
    isStepVisible(s.step.id, headSet, stepMap, opts.expandedBranches),
  );

  // Pass 1 — render cards so the browser can measure them.
  nodesEl.innerHTML = visible
    .map((s) => {
      const id = s.step.id;
      return renderCard(s, {
        isHead: id === head,
        isDead: !headSet.has(id),
        isFocused: id === opts.selectedStepId,
        deadKids: deadChildrenCount(id, headSet, childrenMap),
        isExpanded: !!opts.expandedBranches[id],
        actors,
        showTs: opts.showTs,
        showFiles: opts.showFiles,
      });
    })
    .join("");

  // Pass 2 — measure.
  const dims = new Map<string, { width: number; height: number }>();
  for (const s of visible) {
    const el = document.getElementById(`pg-card-${cssSafeId(s.step.id)}`);
    if (!el) continue;
    dims.set(s.step.id, {
      width: el.offsetWidth,
      height: el.offsetHeight,
    });
  }

  // Pass 3 — dagre layout.
  const g = new dagre.graphlib.Graph();
  g.setGraph({
    rankdir: "TB",
    nodesep: 30,
    ranksep: 48,
    marginx: 16,
    marginy: 16,
  });
  g.setDefaultEdgeLabel(() => ({}));
  for (const s of visible) {
    const d = dims.get(s.step.id);
    if (!d) continue;
    g.setNode(s.step.id, d);
  }
  const visibleIds = new Set(visible.map((s) => s.step.id));
  for (const s of visible) {
    const id = s.step.id;
    for (const p of s.step.parents || []) {
      if (!visibleIds.has(p)) continue;
      const childIsDead = !headSet.has(id);
      g.setEdge(p, id, { dead: childIsDead });
    }
  }
  dagre.layout(g);
  const gi = g.graph();

  // Pass 4 — size + position.
  const w = Math.ceil(gi.width ?? 0);
  const h = Math.ceil(gi.height ?? 0);
  graphEl.style.width = w + "px";
  graphEl.style.height = h + "px";
  svgEl.setAttribute("width", String(w));
  svgEl.setAttribute("height", String(h));
  for (const s of visible) {
    const n = g.node(s.step.id);
    if (!n) continue;
    const el = document.getElementById(`pg-card-${cssSafeId(s.step.id)}`);
    if (!el) continue;
    el.style.left = n.x - n.width / 2 + "px";
    el.style.top = n.y - n.height / 2 + "px";
    el.style.visibility = "visible";
  }

  // Pass 5 — edges.
  for (const path of Array.from(svgEl.querySelectorAll("path.path-graph__edge"))) {
    path.remove();
  }
  for (const e of g.edges()) {
    const edge = g.edge(e) as { points: { x: number; y: number }[]; dead?: boolean };
    if (!edge?.points?.length) continue;
    const d = pointsToPath(edge.points);
    const path = document.createElementNS(NS, "path");
    path.setAttribute("d", d);
    path.setAttribute(
      "class",
      "path-graph__edge" + (edge.dead ? " path-graph__edge--dead" : ""),
    );
    path.setAttribute(
      "marker-end",
      edge.dead ? "url(#pg-arrow-dead)" : "url(#pg-arrow)",
    );
    svgEl.appendChild(path);
  }

  // Click delegation
  nodesEl.onclick = (ev: MouseEvent) => {
    const target = ev.target as HTMLElement;
    const toggle = target.closest<HTMLElement>("[data-toggle-branch]");
    if (toggle) {
      ev.stopPropagation();
      const id = toggle.getAttribute("data-toggle-branch")!;
      opts.onToggleBranch(id);
      return;
    }
    const card = target.closest<HTMLElement>("[data-step-id]");
    if (!card) return;
    const id = card.getAttribute("data-step-id")!;
    const step = stepMap.get(id);
    if (!step) return;
    opts.onSelectStep(step, actors);
  };
}
