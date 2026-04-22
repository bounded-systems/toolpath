import { describe, expect, it } from "vitest";
import { classify, readStructural } from "../classify";
import type { StepRef } from "../types";

// Minimal StepRef fixture — only the fields classify/readStructural read.
// `structural` is the flattened wire shape (see StructuralChange in Rust:
// `#[serde(rename = "type")]` on the tag + `#[serde(flatten)]` on extra).
function stepWith(structural: Record<string, unknown>): StepRef {
  return {
    step: { id: "s", actor: "agent:claude-code" },
    change: { "convo://turn": { structural } },
  };
}

describe("classify", () => {
  it("returns 'user' for a conversation.append with role=user", () => {
    const s = stepWith({ type: "conversation.append", role: "user", text: "hi" });
    expect(classify(s).kind).toBe("user");
  });

  it("returns 'assistant' for a conversation.append with role=assistant", () => {
    const s = stepWith({
      type: "conversation.append",
      role: "assistant",
      text: "hello",
    });
    expect(classify(s).kind).toBe("assistant");
  });

  it("returns 'tool' for tool.invoke", () => {
    const s = stepWith({ type: "tool.invoke", name: "Edit" });
    const c = classify(s);
    expect(c.kind).toBe("tool");
    expect(c.toolName).toBe("Edit");
  });

  it("returns 'system' for unknown types", () => {
    const s = stepWith({ type: "conversation.init", model: "claude-opus-4-7" });
    expect(classify(s).kind).toBe("system");
  });
});

describe("readStructural", () => {
  it("prefers a tool.invoke payload over a non-conversation sibling artifact", () => {
    // Claude multi-artifact case: a tool.invoke step also writes a file
    // artifact whose `structural` is something unrelated (not in the
    // CONVERSATION_TYPES set). readStructural should pick the tool.invoke.
    const step: StepRef = {
      step: { id: "s", actor: "agent:claude-code" },
      change: {
        "file:///tmp/out.txt": {
          structural: { type: "file.write", bytes: 42 },
        },
        "tool://Edit": {
          structural: { type: "tool.invoke", name: "Edit" },
        },
      },
    };
    const s = readStructural(step);
    expect(s?.type).toBe("tool.invoke");
    expect(s?.name).toBe("Edit");
  });
});
