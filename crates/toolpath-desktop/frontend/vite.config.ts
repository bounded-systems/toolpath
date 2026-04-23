import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

// Tauri expects a fixed port; strictPort = fail if taken rather than pick
// another one behind its back.
const HOST = "127.0.0.1";
const PORT = 1420;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: HOST,
    port: PORT,
    strictPort: true,
    hmr: { protocol: "ws", host: HOST, port: PORT + 1 },
    watch: {
      // Don't let Vite re-scan the Rust side.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
  build: {
    target: "es2022",
    // Tauri bundles the assets; a single-chunk build keeps the devtools
    // panel readable and avoids weird code-splitting behaviour in the
    // webview.
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      // Two HTML entries: the main window (index.html) and the tray popover
      // (popover.html). Both are bundled into frontend/dist by Tauri.
      input: {
        main: resolve(__dirname, "index.html"),
        popover: resolve(__dirname, "popover.html"),
      },
    },
  },
});
