// Reactive store wrapping the pure `update` reducer. Exposes a single
// `store.m` proxy that Svelte components can read reactively, plus a
// `dispatch` function.

import type { Cmd, Dispatch, Model, Msg } from "./types";
import { initialModel, update } from "./update";
import { invoke } from "./ipc";
import { dbg } from "./debug";
import { perfStart, perfMark, perfEnd } from "./perf.svelte";

class Store {
  m = $state<Model>(initialModel());

  dispatch: Dispatch = (msg: Msg) => {
    dbg("msg", msg.t, msg);
    // Perf: a derive-dispatch begins a new trace; `DeriveSucceeded` lands
    // the model update that triggers the preview route to mount.
    if (msg.t === "ClaudeDerive") perfStart("derive claude");
    else if (msg.t === "PiDerive") perfStart("derive pi");
    if (msg.t === "ClaudeDerive" || msg.t === "PiDerive") perfMark("dispatch");
    if (msg.t === "DeriveSucceeded") perfMark("model-updated");
    const [next, cmd] = update(msg, this.m);
    const routeChanged = next.route !== this.m.route;
    this.m = next;
    if (routeChanged) dbg("route", next.route);
    // Cmds run synchronously. For streaming flows where the backend emits
    // events, the component owns the subscribe-then-invoke sequencing (see
    // BrowseClaude / BrowsePi $effect blocks) so listeners are confirmed
    // live before the invoke fires.
    if (cmd) this.runCmd(cmd);
  };

  private runCmd(cmd: Cmd): void {
    switch (cmd.type) {
      case "batch":
        for (const c of cmd.cmds) this.runCmd(c);
        return;
      case "emitMsg":
        queueMicrotask(() => this.dispatch(cmd.msg));
        return;
      case "fn":
        void cmd.run(this.dispatch);
        return;
      case "invoke":
        dbg("invoke", cmd.name, cmd.args ?? {});
        if (cmd.name === "derive_claude" || cmd.name === "derive_pi") {
          perfMark("invoke-start");
        }
        invoke(cmd.name, cmd.args).then(
          (r) => {
            dbg("invoke.ok", cmd.name, r);
            if (cmd.name === "derive_claude" || cmd.name === "derive_pi") {
              perfMark("invoke-end");
            }
            const m = cmd.onOk?.(r);
            if (m) this.dispatch(m);
          },
          (e) => {
            dbg("invoke.err", cmd.name, e);
            if (cmd.name === "derive_claude" || cmd.name === "derive_pi") {
              perfMark("invoke-err");
              perfEnd();
            }
            const m = cmd.onErr?.(e);
            if (m) this.dispatch(m);
          },
        );
        return;
    }
  }
}

export const store = new Store();
