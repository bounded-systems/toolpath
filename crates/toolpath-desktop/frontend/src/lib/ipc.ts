// Tauri IPC bridge wrappers, typed.
//
// `invoke` is a thin typed wrapper around `@tauri-apps/api/core`. Rust
// commands are snake_case; Tauri converts camelCase JS args to snake_case
// Rust params.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { dbg } from "./debug";

export function invoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return tauriInvoke<T>(cmd, args);
}

/**
 * Subscribe to a Tauri backend event. Returns a promise of the unlisten
 * function so `$effect` can clean up automatically. Every received event is
 * logged via `dbg("event", name, payload)`.
 */
export function listen<T = unknown>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  dbg("event", event + " (subscribed)");
  return tauriListen<T>(event, (ev) => {
    dbg("event", event, ev.payload);
    handler(ev.payload as T);
  });
}
