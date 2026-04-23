// DFS flatten of a Toolpath Document into a sidebar-friendly tree, with
// gutter bookkeeping so rows can render ascii tree connectors (`├─`, `└─`,
// `│`). Reorders siblings so the HEAD-path child comes last — this keeps
// the "live" branch visually anchored on the left/bottom rail.
//
// Also exposes `flattenChatHead(norm)`, a linear earliest→latest sequence
// of the HEAD-path steps used by the chat/transcript view.

import { classify, type ChatTurnKind } from "./classify";
import { renderMarkdown } from "./markdown";
import { perfMark } from "./perf.svelte";
import type { StepRef, TreeFilter } from "./types";
import { actorName, actorType, normalize, type Normalized } from "./viz";

export type { ChatTurnKind };

export interface FlatNode {
  id: string;
  depth: number;
  /** One of "├─ ", "└─ ", or "" (root rows). */
  prefix: string;
  onHead: boolean;
  isHead: boolean;
  isDead: boolean;
  actor: string;
  actorDisplay: string;
  actorKind: string;
  intent: string;
}

export function flattenTree(norm: Normalized): FlatNode[] {
  const { steps, head, actors, stepMap, childrenMap, headSet } = norm;

  // Root set: any step whose parent is missing from the doc (or has none).
  const hasParentInDoc = (id: string) => {
    const s = stepMap.get(id);
    const ps = s?.step.parents ?? [];
    return ps.some((p) => stepMap.has(p));
  };
  const roots = steps.map((s) => s.step.id).filter((id) => !hasParentInDoc(id));

  const reorder = (kids: string[]): string[] => {
    // HEAD child last, so DFS visits dead branches first. Combined with the
    // prototype's "last sibling inherits parent lane" convention, this also
    // anchors the main/live branch on the rail.
    const headKids: string[] = [];
    const other: string[] = [];
    for (const k of kids) (headSet.has(k) ? headKids : other).push(k);
    return [...other, ...headKids];
  };

  const out: FlatNode[] = [];
  const visit = (id: string, depth: number, gutters: boolean[], isLast: boolean) => {
    const s = stepMap.get(id);
    if (!s) return;
    const prefix = buildPrefix(depth, gutters, isLast);
    const onHead = headSet.has(id);
    const isHead = id === head;
    const kind = actorType(s.step.actor);
    out.push({
      id,
      depth,
      prefix,
      onHead,
      isHead,
      isDead: !onHead,
      actor: s.step.actor,
      actorDisplay: actorName(s.step.actor, actors),
      actorKind: kind,
      intent: s.meta?.intent ?? "",
    });
    const kids = reorder(childrenMap.get(id) ?? []);
    const nextGutters = [...gutters, !isLast];
    kids.forEach((k, i) => visit(k, depth + 1, nextGutters, i === kids.length - 1));
  };

  const reorderedRoots = reorder(roots);
  reorderedRoots.forEach((r, i) =>
    visit(r, 0, [], i === reorderedRoots.length - 1),
  );
  return out;
}

function buildPrefix(depth: number, gutters: boolean[], isLast: boolean): string {
  if (depth === 0) return "";
  let s = "";
  for (let lvl = 0; lvl < depth - 1; lvl++) {
    s += gutters[lvl] ? "│  " : "   ";
  }
  s += isLast ? "└─ " : "├─ ";
  return s;
}

export function matchesFilter(
  node: FlatNode,
  query: string,
  filter: TreeFilter,
): boolean {
  if (filter === "head" && !node.onHead) return false;
  if (filter === "dead" && node.onHead) return false;
  if (query) {
    const hay = `${node.id} ${node.actor} ${node.intent}`.toLowerCase();
    if (!hay.includes(query)) return false;
  }
  return true;
}

/** Convenience: normalize + flatten in one call. */
/**
 * `buildTree` is called from multiple `$derived` blocks (StepTree +
 * ChatView), so without memoisation the same doc gets normalised + flattened
 * twice on every preview open. Memo by doc identity — callers only ever pass
 * `store.m.preview.doc` which is a stable reference between re-renders. A
 * `WeakMap` keeps the cache GC-friendly: once the doc is replaced (new
 * derive) the old entry is collectable.
 */
type BuiltTree = { norm: Normalized; nodes: FlatNode[] };
const treeCache = new WeakMap<object, BuiltTree>();

export function buildTree(
  doc: Parameters<typeof normalize>[0],
): BuiltTree {
  // `doc` is a JSON object at the top level (Step/Path/Graph wrapper), so
  // WeakMap can key on it directly.
  const cached = treeCache.get(doc as object);
  if (cached) {
    perfMark(`buildTree cache-hit (${cached.norm.steps.length}st)`);
    return cached;
  }
  const t0 = performance.now();
  const norm = normalize(doc);
  const tNorm = performance.now() - t0;
  const nodes = flattenTree(norm);
  const tTotal = performance.now() - t0;
  perfMark(
    `buildTree (${norm.steps.length}st ${tTotal.toFixed(0)}ms: norm ${tNorm.toFixed(0)} + flat ${(tTotal - tNorm).toFixed(0)})`,
  );
  const built = { norm, nodes };
  treeCache.set(doc as object, built);
  return built;
}

// ─── Chat / transcript view ──────────────────────────────────────────────

export interface ChatTurn {
  id: string;
  step: StepRef;
  actor: string;
  actorDisplay: string;
  actorKind: string;
  intent: string;
  timestamp: string | null;
  isHead: boolean;
  changeKeys: string[];
  /** Derived from structural.change_type + extra.role. */
  kind: ChatTurnKind;
  /** User / assistant text (from change[k].structural.extra.text). */
  text: string | null;
  /** Sanitized markdown-rendered HTML for {@link text}. Precomputed at
   *  flatten time so re-renders of the chat list (which happen on any
   *  unrelated state change — filter typing, selection, view toggle) skip
   *  the `marked.parse + DOMPurify.sanitize` pipeline. Empty string if
   *  `text` is null/empty. */
  textHtml: string;
  /** Summary of tool names in this turn (from extra.tool_uses). */
  toolNames: string[];
  /** Thinking-block text, if captured (from extra.thinking). */
  thinking: string | null;
  /** Sanitized markdown-rendered HTML for {@link thinking}. Precomputed
   *  for the same reason as {@link textHtml}. Empty string if `thinking`
   *  is null/empty. */
  thinkingHtml: string;
  /** Model identifier (e.g. `claude-opus-4-6`). */
  model: string | null;
  /** For `kind === "tool"` only: the name of the invoked tool. */
  toolName: string | null;
  /** For `kind === "tool"`: the first non-empty `change[k].raw` on the
   *  step — file-write tools carry a unified diff there. Precomputed at
   *  flatten time so the diff body doesn't re-split on every render. */
  toolDiff: { path: string; raw: string; lines: string[] } | null;
  /** For `kind === "assistant"`: the `tool.invoke` sibling steps spawned
   *  by this assistant turn (Claude derives them as separate siblings
   *  rather than linear descendants). Rendered inline inside the bubble. */
  toolInvocations: ChatTurn[];
}


/** Pick the first non-empty `change[k].raw` on a step — file-write tools
 *  (Edit/Write) carry the unified diff there. Precomputed once per turn
 *  at flatten time so the chat view doesn't re-split on every render. */
function firstRawDiff(
  s: StepRef,
): { path: string; raw: string; lines: string[] } | null {
  const ch = s.change;
  if (!ch) return null;
  for (const key of Object.keys(ch)) {
    const raw = ch[key]?.raw;
    if (typeof raw === "string" && raw.length) {
      return { path: key, raw, lines: raw.split("\n") };
    }
  }
  return null;
}

/**
 * Linearise the HEAD-ancestor chain earliest → latest for a chat-style
 * transcript. Follows `parents[0]` up from `head`, then reverses. Steps
 * with no `head` (bare Step docs, or Paths without a head) fall back to
 * the full step list in declaration order.
 *
 * Also splices in each HEAD-chain step's `tool.invoke` children right
 * after it — those attach to an assistant step as siblings (they don't
 * advance HEAD), so a naive parent-walk would skip them and you'd see
 * "Used Edit" chips with no actual tool output in the transcript.
 */
// Memo by norm identity. Same rationale as buildTree — ChatView's `$derived`
// block calls this, and it's expensive (markdown rendering dominates).
const flattenCache = new WeakMap<Normalized, ChatTurn[]>();

export function flattenChatHead(norm: Normalized): ChatTurn[] {
  const cached = flattenCache.get(norm);
  if (cached) {
    perfMark(`flattenChatHead cache-hit (${cached.length}t)`);
    return cached;
  }
  const { steps, head, actors, stepMap, childrenMap } = norm;
  const t0 = performance.now();

  let ordered: StepRef[];
  if (head && stepMap.has(head)) {
    const chain: StepRef[] = [];
    const seen = new Set<string>();
    let cursor: string | undefined = head;
    while (cursor && !seen.has(cursor)) {
      seen.add(cursor);
      const s = stepMap.get(cursor);
      if (!s) break;
      chain.push(s);
      cursor = s.step.parents?.[0];
    }
    chain.reverse();

    ordered = chain;
  } else {
    ordered = steps;
  }

  // Build each turn. For assistant turns, also collect tool.invoke sibling
  // children so the renderer can fold them inline inside the bubble instead
  // of scattering them as separate cards in the transcript.
  let mdMs = 0;
  let mdCount = 0;
  const timedMarkdown = (src: string | null | undefined): string => {
    if (!src) return "";
    const t = performance.now();
    const out = renderMarkdown(src);
    mdMs += performance.now() - t;
    mdCount += 1;
    return out;
  };
  const turnFor = (s: StepRef): ChatTurn => {
    const c = classify(s);
    return {
      id: s.step.id,
      step: s,
      actor: s.step.actor,
      actorDisplay: actorName(s.step.actor, actors),
      actorKind: actorType(s.step.actor),
      intent: s.meta?.intent ?? "",
      timestamp: s.step.timestamp ?? null,
      isHead: s.step.id === head,
      changeKeys: s.change ? Object.keys(s.change) : [],
      kind: c.kind,
      text: c.text,
      textHtml: timedMarkdown(c.text),
      toolNames: c.toolNames,
      thinking: c.thinking,
      thinkingHtml: timedMarkdown(c.thinking),
      model: c.model,
      toolName: c.toolName,
      toolDiff: firstRawDiff(s),
      toolInvocations: [],
    };
  };

  const onChain = new Set(ordered.map((s) => s.step.id));
  const out = ordered.map((s) => {
    const turn = turnFor(s);
    if (turn.kind === "assistant") {
      const kids = childrenMap.get(s.step.id) ?? [];
      for (const kid of kids) {
        if (onChain.has(kid)) continue;
        const child = stepMap.get(kid);
        if (!child) continue;
        const childTurn = turnFor(child);
        if (childTurn.kind === "tool") turn.toolInvocations.push(childTurn);
      }
    }
    return turn;
  });
  const total = performance.now() - t0;
  perfMark(
    `flattenChatHead (${out.length}t ${total.toFixed(0)}ms: md ${mdMs.toFixed(0)}ms × ${mdCount})`,
  );
  flattenCache.set(norm, out);
  return out;
}
