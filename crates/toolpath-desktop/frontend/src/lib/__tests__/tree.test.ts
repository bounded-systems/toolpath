import { describe, expect, it } from "vitest";
import { flattenChatHead } from "../tree";
import type { Document, StepRef } from "../types";
import { normalize } from "../viz";

// ── Step fixture helpers ───────────────────────────────────────────────

function userStep(id: string, parent: string | null, text: string): StepRef {
  return {
    step: { id, actor: "human:alex", parents: parent ? [parent] : [] },
    change: {
      "convo://turn": {
        structural: { type: "conversation.append", role: "user", text },
      },
    },
  };
}

function assistantStep(id: string, parent: string, text: string): StepRef {
  return {
    step: { id, actor: "agent:claude-code", parents: [parent] },
    change: {
      "convo://turn": {
        structural: { type: "conversation.append", role: "assistant", text },
      },
    },
  };
}

function toolStep(id: string, parent: string, name: string): StepRef {
  return {
    step: { id, actor: "agent:claude-code", parents: [parent] },
    change: {
      "tool://invoke": {
        structural: { type: "tool.invoke", name },
      },
    },
  };
}

function pathDoc(head: string, steps: StepRef[]): Document {
  return {
    Path: {
      path: { id: "p", head },
      steps,
    },
  };
}

// ── Tests ──────────────────────────────────────────────────────────────

describe("flattenChatHead", () => {
  it("linearises a user → assistant → user → assistant chain in order", () => {
    const steps = [
      userStep("s1", null, "hi"),
      assistantStep("s2", "s1", "hello"),
      userStep("s3", "s2", "how are you"),
      assistantStep("s4", "s3", "good"),
    ];
    const turns = flattenChatHead(normalize(pathDoc("s4", steps)));
    expect(turns.map((t) => t.id)).toEqual(["s1", "s2", "s3", "s4"]);
    expect(turns.map((t) => t.kind)).toEqual([
      "user",
      "assistant",
      "user",
      "assistant",
    ]);
  });

  it("attaches a tool.invoke sibling child to its assistant parent", () => {
    // s2 (assistant) has two children: s3 (tool.invoke sibling, not on HEAD)
    // and s4 (next user turn, on HEAD). head = s4.
    const steps = [
      userStep("s1", null, "hi"),
      assistantStep("s2", "s1", "ok"),
      toolStep("s3", "s2", "Edit"),
      userStep("s4", "s2", "thanks"),
    ];
    const turns = flattenChatHead(normalize(pathDoc("s4", steps)));
    const s2 = turns.find((t) => t.id === "s2")!;
    expect(s2.toolInvocations.map((t) => t.id)).toEqual(["s3"]);
    expect(s2.toolInvocations[0].toolName).toBe("Edit");
    // s3 should not appear as its own top-level turn.
    expect(turns.map((t) => t.id)).toEqual(["s1", "s2", "s4"]);
  });

  it("ignores sibling children that aren't tool.invoke", () => {
    const steps = [
      userStep("s1", null, "hi"),
      assistantStep("s2", "s1", "ok"),
      // sibling child that's a user turn, not a tool.invoke
      userStep("s3", "s2", "detour"),
      userStep("s4", "s2", "thanks"),
    ];
    const turns = flattenChatHead(normalize(pathDoc("s4", steps)));
    const s2 = turns.find((t) => t.id === "s2")!;
    expect(s2.toolInvocations).toEqual([]);
  });

  it("falls back to the full step list for a bare Step doc (no head)", () => {
    const doc: Document = {
      Step: userStep("solo", null, "hi") as StepRef & {
        meta?: { actors?: Record<string, never> };
      },
    };
    const turns = flattenChatHead(normalize(doc));
    expect(turns.map((t) => t.id)).toEqual(["solo"]);
  });

  it("falls back to the full step list when head is detached (not in stepMap)", () => {
    const steps = [
      userStep("s1", null, "hi"),
      assistantStep("s2", "s1", "ok"),
    ];
    // head points to a non-existent step.
    const turns = flattenChatHead(normalize(pathDoc("missing", steps)));
    expect(turns.map((t) => t.id)).toEqual(["s1", "s2"]);
  });
});
