// Minimal markdown → sanitized HTML renderer for chat bodies. Pipes
// `marked` through DOMPurify so raw `<script>` / event-handler attributes
// smuggled into Claude output can't execute in the Tauri webview.
//
// Configured with GitHub-style linebreaks (single-newline → `<br>`) so
// terse chat messages read naturally without blank lines between them.

import DOMPurify from "dompurify";
import { marked } from "marked";

marked.setOptions({
  gfm: true,
  breaks: true,
});

export function renderMarkdown(src: string | null | undefined): string {
  if (!src) return "";
  const html = marked.parse(src, { async: false }) as string;
  return DOMPurify.sanitize(html, { USE_PROFILES: { html: true } });
}
