// Lightweight debug-log helper.
//
// `dbg("msg", name, payload)` prints a timestamped, colour-coded line to the
// webview devtools console. Category is a string tag (`msg`, `route`,
// `invoke`, `invoke.ok`, `invoke.err`, `event`, ...). Disable all output by
// setting `localStorage.debug = "0"` or `false`; default is on in dev.
//
// Pretty-prints non-trivial payloads as expandable objects rather than
// stringifying them, so the devtools inspector stays useful.

const COLOURS: Record<string, string> = {
  msg: "#b5652b",
  route: "#3a7e3a",
  invoke: "#5b7bc4",
  "invoke.ok": "#3a7e3a",
  "invoke.err": "#c44030",
  event: "#8a5a9a",
  default: "#8a8078",
};

function enabled(): boolean {
  try {
    const v = globalThis.localStorage?.getItem("debug");
    return v !== "0" && v !== "false";
  } catch {
    return true;
  }
}

function ts(): string {
  const d = new Date();
  return (
    String(d.getHours()).padStart(2, "0") +
    ":" +
    String(d.getMinutes()).padStart(2, "0") +
    ":" +
    String(d.getSeconds()).padStart(2, "0") +
    "." +
    String(d.getMilliseconds()).padStart(3, "0")
  );
}

export function dbg(category: string, label: string, ...rest: unknown[]): void {
  if (!enabled()) return;
  const colour = COLOURS[category] ?? COLOURS.default;
  // eslint-disable-next-line no-console
  console.log(
    `%c${ts()} %c${category}%c ${label}`,
    "color:#8a8078;font-family:monospace",
    `color:${colour};font-weight:600;font-family:monospace`,
    "color:inherit;font-family:monospace",
    ...rest,
  );
}
