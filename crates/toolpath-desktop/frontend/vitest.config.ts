import { defineConfig } from "vitest/config";

// Pure-TS unit tests for the lib modules. No Svelte component tests, no
// DOM — the two modules we test (`classify.ts`, `tree.ts`) operate on
// plain data shapes, so the Node environment is sufficient.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
