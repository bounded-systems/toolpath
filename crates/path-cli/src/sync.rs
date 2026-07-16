//! [`ArtifactType`], the single enum naming the artifact sources the
//! CLI operates over.

/// The kind of artifact an operation ranges over. One enum, used
/// everywhere a command names artifact sources (`share`/`resume`
/// `--harness` via the [`Harness`](crate::cmd_share::Harness) layer,
/// import cache-id prefixes); `name()` doubles as the cache-id prefix.
/// Github and pathbase are absent on purpose: they are remote
/// services, not local artifact sources.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, clap::ValueEnum)]
#[value(rename_all = "lower")]
pub enum ArtifactType {
    Claude,
    Gemini,
    Codex,
    Opencode,
    Cursor,
    Pi,
    Copilot,
    Git,
}

impl ArtifactType {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            ArtifactType::Claude => "claude",
            ArtifactType::Gemini => "gemini",
            ArtifactType::Codex => "codex",
            ArtifactType::Opencode => "opencode",
            ArtifactType::Cursor => "cursor",
            ArtifactType::Pi => "pi",
            ArtifactType::Copilot => "copilot",
            ArtifactType::Git => "git",
        }
    }

    /// Padded so all symbols line up in the fzf column. Longest is
    /// "opencode" (8); pad shorter names to match.
    pub(crate) fn symbol(&self) -> &'static str {
        match self {
            ArtifactType::Claude => "claude  ",
            ArtifactType::Gemini => "gemini  ",
            ArtifactType::Codex => "codex   ",
            ArtifactType::Opencode => "opencode",
            ArtifactType::Cursor => "cursor  ",
            ArtifactType::Pi => "pi      ",
            ArtifactType::Copilot => "copilot ",
            ArtifactType::Git => "git     ",
        }
    }

    /// True when the underlying provider keys artifacts by a filesystem
    /// path (the project directory). claude/gemini/pi: true.
    /// codex/opencode/cursor: false (sessions store cwd per-row, not as
    /// a directory key — cursor stores it as
    /// `workspaceIdentifier.uri.fsPath` on each composer).
    pub(crate) fn path_keyed(&self) -> bool {
        matches!(
            self,
            ArtifactType::Claude | ArtifactType::Gemini | ArtifactType::Pi | ArtifactType::Git
        )
    }

    pub(crate) fn parse(s: &str) -> Option<Self> {
        <Self as clap::ValueEnum>::from_str(s, false).ok()
    }
}

#[cfg(test)]
mod type_tests {
    use super::ArtifactType;

    /// Every artifact type, in presentation order.
    const ALL: [ArtifactType; 8] = [
        ArtifactType::Claude,
        ArtifactType::Gemini,
        ArtifactType::Codex,
        ArtifactType::Opencode,
        ArtifactType::Cursor,
        ArtifactType::Pi,
        ArtifactType::Copilot,
        ArtifactType::Git,
    ];

    #[test]
    fn names_and_symbols_are_distinct() {
        let names: std::collections::HashSet<&str> = ALL.iter().map(|t| t.name()).collect();
        let symbols: std::collections::HashSet<&str> = ALL.iter().map(|t| t.symbol()).collect();
        assert_eq!(names.len(), ALL.len());
        assert_eq!(symbols.len(), ALL.len());
    }

    #[test]
    fn path_keyed_matches_design() {
        assert!(ArtifactType::Claude.path_keyed());
        assert!(ArtifactType::Gemini.path_keyed());
        assert!(ArtifactType::Pi.path_keyed());
        assert!(!ArtifactType::Codex.path_keyed());
        assert!(!ArtifactType::Opencode.path_keyed());
        assert!(!ArtifactType::Cursor.path_keyed());
        assert!(ArtifactType::Git.path_keyed());
    }

    #[test]
    fn parse_roundtrips_every_name() {
        for t in ALL {
            assert_eq!(ArtifactType::parse(t.name()), Some(t));
        }
        assert_eq!(ArtifactType::parse("frobnicate"), None);
    }
}
