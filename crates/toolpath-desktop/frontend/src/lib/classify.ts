// Step-level classification shared by the chat transcript and the graph
// card renderer. Reads the `structural` payload of a step's change entries
// and extracts role / text / tool metadata.
//
// Keep in sync with `crates/toolpath/src/types.rs::StructuralChange`:
//   - `change_type` is `#[serde(rename = "type")]`
//   - `extra: HashMap<String, Value>` is `#[serde(flatten)]`
// so on the wire the payload is a flat object like
// `{ type: "conversation.append", role: "assistant", text: "...", ... }`.

import type { StepRef } from "./types";

export type ChatTurnKind =
  | "user"        // conversation.append, role=user
  | "assistant"   // conversation.append, role=assistant
  | "tool"        // tool.invoke
  | "system"      // conversation.init / conversation.event / anything else
  ;

export type StructuralPayload = Record<string, unknown> & { type?: string };

const CONVERSATION_TYPES = new Set([
  "conversation.append",
  "conversation.init",
  "conversation.event",
  "tool.invoke",
]);

/**
 * Pick the best `structural` payload from a step's change map — prefer
 * one whose `type` is a known conversation variant so that (e.g.) a tool
 * file artifact doesn't shadow the conversation payload.
 */
export function readStructural(step: StepRef): StructuralPayload | null {
  if (!step.change) return null;
  const candidates: StructuralPayload[] = [];
  for (const key of Object.keys(step.change)) {
    const s = step.change[key]?.structural as StructuralPayload | undefined;
    if (s && typeof s === "object") candidates.push(s);
  }
  if (candidates.length === 0) return null;
  return (
    candidates.find((c) => typeof c.type === "string" && CONVERSATION_TYPES.has(c.type))
    ?? candidates[0]
  );
}

function asString(v: unknown): string | null {
  return typeof v === "string" && v.length ? v : null;
}

function asStringArray(v: unknown): string[] {
  if (!Array.isArray(v)) return [];
  return v.filter((x): x is string => typeof x === "string");
}

export interface Classified {
  kind: ChatTurnKind;
  text: string | null;
  toolNames: string[];
  thinking: string | null;
  model: string | null;
  toolName: string | null;
}

export function classify(step: StepRef): Classified {
  const s = readStructural(step) ?? {};
  const ct = typeof s.type === "string" ? s.type : "";
  const role = asString(s["role"]);
  const text = asString(s["text"]);
  const thinking = asString(s["thinking"]);
  const toolNames = asStringArray(s["tool_uses"]);
  const model = asString(s["model"]);
  const toolName = asString(s["name"]);

  let kind: ChatTurnKind;
  if (ct === "tool.invoke") kind = "tool";
  else if (ct === "conversation.append") {
    kind = role === "user" ? "user" : "assistant";
  } else {
    kind = "system";
  }
  return { kind, text, toolNames, thinking, model, toolName };
}
