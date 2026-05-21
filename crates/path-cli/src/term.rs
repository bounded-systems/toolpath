//! Render the small Markdown subset `path show` emits into ANSI for terminal
//! display (the fzf preview panes). fzf interprets ANSI escapes in previews
//! but not Markdown, so without this the panes show raw `**`/`#`/`*` markers.
//!
//! This handles only the constructs the transcript renderer produces:
//! headings, `**bold**` / `*dim*` / `` `code` `` inline spans, `>` blockquotes,
//! and fenced code blocks (diff blocks get red/green +/- lines). It is not a
//! general Markdown engine.

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";

/// Convert the transcript Markdown to ANSI-styled text.
pub fn markdown_to_ansi(md: &str) -> String {
    let mut out = String::new();
    let mut fence_lang: Option<String> = None;

    for line in md.lines() {
        if let Some(rest) = line.strip_prefix("```") {
            fence_lang = match fence_lang {
                None => Some(rest.trim().to_string()),
                Some(_) => None,
            };
            continue; // drop the fence marker line itself
        }

        if let Some(lang) = &fence_lang {
            let styled = if lang == "diff" {
                if line.starts_with('+') {
                    format!("{GREEN}{line}{RESET}")
                } else if line.starts_with('-') {
                    format!("{RED}{line}{RESET}")
                } else {
                    format!("{DIM}{line}{RESET}")
                }
            } else {
                format!("{DIM}{line}{RESET}")
            };
            out.push_str(&styled);
            out.push('\n');
            continue;
        }

        if let Some(h) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
            .or_else(|| line.strip_prefix("# "))
        {
            out.push_str(&format!("{BOLD}{}{RESET}\n", inline(h)));
            continue;
        }

        if let Some(q) = line.strip_prefix("> ") {
            out.push_str(&format!("{DIM}\u{2502} {q}{RESET}\n"));
            continue;
        }

        out.push_str(&inline(line));
        out.push('\n');
    }
    out
}

/// Replace inline `**bold**`, `*dim*`, and `` `code` `` spans with ANSI,
/// stripping the markers.
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("**")
            && let Some(end) = r.find("**")
        {
            out.push_str(BOLD);
            out.push_str(&inline(&r[..end]));
            out.push_str(RESET);
            rest = &r[end + 2..];
            continue;
        }
        if let Some(r) = rest.strip_prefix('`')
            && let Some(end) = r.find('`')
        {
            out.push_str(CYAN);
            out.push_str(&r[..end]);
            out.push_str(RESET);
            rest = &r[end + 1..];
            continue;
        }
        if let Some(r) = rest.strip_prefix('*')
            && let Some(end) = r.find('*')
        {
            out.push_str(DIM);
            out.push_str(&r[..end]);
            out.push_str(RESET);
            rest = &r[end + 1..];
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_dim_code_spans() {
        let out = inline("**User:** ran `ls` *(quietly)*");
        assert!(out.contains(&format!("{BOLD}User:{RESET}")));
        assert!(out.contains(&format!("{CYAN}ls{RESET}")));
        assert!(out.contains(&format!("{DIM}(quietly){RESET}")));
        assert!(!out.contains('*'));
        assert!(!out.contains('`'));
    }

    #[test]
    fn heading_becomes_bold_without_hashes() {
        let out = markdown_to_ansi("# Title\n");
        assert!(out.contains(&format!("{BOLD}Title{RESET}")));
        assert!(!out.contains('#'));
    }

    #[test]
    fn diff_fence_colors_and_drops_markers() {
        let md = "```diff\n+added\n-removed\n context\n```\n";
        let out = markdown_to_ansi(md);
        assert!(out.contains(&format!("{GREEN}+added{RESET}")));
        assert!(out.contains(&format!("{RED}-removed{RESET}")));
        assert!(!out.contains("```"));
    }

    #[test]
    fn lone_asterisk_is_left_alone() {
        let out = inline("2 * 3 = 6");
        assert_eq!(out, "2 * 3 = 6");
    }
}
