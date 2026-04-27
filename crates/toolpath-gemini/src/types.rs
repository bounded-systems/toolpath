//! On-disk schema for Gemini CLI conversation files, plus the higher-level
//! `Conversation` type that bundles a main chat with its sibling sub-agent
//! chats.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// The raw JSON chat file as written by Gemini CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFile {
    /// Short alphanumeric identifier written inside the file.
    /// Distinct from the enclosing session UUID directory name.
    #[serde(default)]
    pub session_id: String,

    /// SHA-256 hex of the absolute project path.
    #[serde(default)]
    pub project_hash: String,

    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,

    #[serde(default)]
    pub last_updated: Option<DateTime<Utc>>,

    /// Workspace directories captured at session start. `None` preserves
    /// "field absent in source" for round-trip fidelity; `Some(vec![])`
    /// preserves an explicit empty array.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directories: Option<Vec<PathBuf>>,

    /// Present on sub-agent chat files (`"subagent"`); absent on the main chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Sub-agent's reported result (populated on `kind: "subagent"` files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    #[serde(default)]
    pub messages: Vec<GeminiMessage>,

    /// Forward-compat: anything not covered above lands here.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single message within a chat file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiMessage {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub timestamp: String,

    /// `"user"` or `"gemini"`.
    #[serde(rename = "type")]
    pub role: GeminiRole,

    #[serde(default)]
    pub content: GeminiContent,

    /// `None` = field absent in source. `Some(vec![])` = explicit empty array.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thoughts: Option<Vec<Thought>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Tokens>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// `None` = field absent in source. `Some(vec![])` = explicit empty array.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "toolCalls")]
    pub tool_calls: Option<Vec<ToolCall>>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl GeminiMessage {
    /// Borrow the thoughts list as a slice, regardless of whether the
    /// source had the field absent or present-but-empty.
    pub fn thoughts(&self) -> &[Thought] {
        self.thoughts.as_deref().unwrap_or(&[])
    }

    /// Borrow the tool-calls list as a slice, regardless of whether the
    /// source had the field absent or present-but-empty.
    pub fn tool_calls(&self) -> &[ToolCall] {
        self.tool_calls.as_deref().unwrap_or(&[])
    }
}

impl ChatFile {
    /// Borrow `directories` as a slice, regardless of absent-vs-empty.
    pub fn directories(&self) -> &[PathBuf] {
        self.directories.as_deref().unwrap_or(&[])
    }
}

/// Who produced a message.
///
/// The three canonical values observed in real Gemini CLI logs are
/// `"user"`, `"gemini"`, and `"info"` (system notifications like
/// "Request cancelled."). Unknown values round-trip through
/// [`GeminiRole::Other`] so the crate stays forward-compatible if
/// Gemini adds new roles.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GeminiRole {
    User,
    Gemini,
    Info,
    Other(String),
}

impl GeminiRole {
    pub fn as_str(&self) -> &str {
        match self {
            GeminiRole::User => "user",
            GeminiRole::Gemini => "gemini",
            GeminiRole::Info => "info",
            GeminiRole::Other(s) => s,
        }
    }
}

impl serde::Serialize for GeminiRole {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for GeminiRole {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(match s.as_str() {
            "user" => GeminiRole::User,
            "gemini" => GeminiRole::Gemini,
            "info" => GeminiRole::Info,
            _ => GeminiRole::Other(s),
        })
    }
}

/// Message content. User messages are typically `Parts([{text}])`; Gemini
/// messages are typically `Text("…")` or `Empty` (with the payload in
/// `toolCalls`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeminiContent {
    Text(String),
    Parts(Vec<TextPart>),
}

impl Default for GeminiContent {
    fn default() -> Self {
        GeminiContent::Text(String::new())
    }
}

impl GeminiContent {
    /// Flattened text content. Empty strings collapse to an empty result.
    pub fn text(&self) -> String {
        match self {
            GeminiContent::Text(s) => s.clone(),
            GeminiContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| p.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// True when the content has no extractable text.
    pub fn is_empty(&self) -> bool {
        match self {
            GeminiContent::Text(s) => s.is_empty(),
            GeminiContent::Parts(parts) => parts
                .iter()
                .all(|p| p.text.as_deref().unwrap_or("").is_empty()),
        }
    }
}

/// One part of a multi-part message. Gemini currently only emits text parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextPart {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// A single thinking step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thought {
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// Per-message token usage. All fields optional because older logs may omit
/// them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tokens {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thoughts: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

/// A tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub args: Value,

    /// `"success"`, `"error"`, or a pending state like `"executing"`.
    #[serde(default)]
    pub status: String,

    #[serde(default)]
    pub timestamp: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result: Vec<FunctionResponse>,

    /// Display-ready result payload. Usually a string, sometimes a
    /// structured value (object with `fileDiff`, styled-text arrays,
    /// etc.). Kept as an opaque `Value` to match whatever Gemini writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_display: Option<Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ToolCall {
    /// If `resultDisplay` is a structured diff (common for
    /// `write_file`/`replace`), return the unified-diff text.
    pub fn file_diff(&self) -> Option<String> {
        self.result_display
            .as_ref()
            .and_then(|v| v.get("fileDiff"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }

    /// Plain-text flavour of `resultDisplay` when it's a plain string
    /// rather than a structured value.
    pub fn result_display_text(&self) -> Option<String> {
        self.result_display
            .as_ref()
            .and_then(|v| v.as_str())
            .map(str::to_string)
    }
}

impl ToolCall {
    /// Flattened text content from all function responses.
    pub fn result_text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for fr in &self.result {
            parts.push(response_to_text(&fr.function_response.response));
        }
        parts.join("\n")
    }

    /// True when this call reported an error status.
    pub fn is_error(&self) -> bool {
        self.status == "error"
    }
}

/// One entry in `toolCalls[].result`, wrapping a Gemini `FunctionResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    #[serde(default, rename = "functionResponse")]
    pub function_response: FunctionResponseBody,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionResponseBody {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub response: Value,
}

fn response_to_text(v: &Value) -> String {
    if let Some(output) = v.get("output").and_then(|o| o.as_str()) {
        return output.to_string();
    }
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if v.is_null() {
        return String::new();
    }
    v.to_string()
}

/// A `logs.json` entry (lightweight per-project prompt log).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub message_id: u64,
    #[serde(default, rename = "type")]
    pub entry_type: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub timestamp: String,
}

/// Lightweight summary of a session on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    /// The session UUID (directory name under `chats/`).
    pub session_uuid: String,
    pub project_path: String,
    pub file_path: PathBuf,
    pub message_count: usize,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity: Option<DateTime<Utc>>,
    /// Number of sub-agent chat files detected alongside the main chat.
    pub sub_agent_count: usize,
    /// The first non-empty user-prompt text in the main chat. Useful as a
    /// human-readable title for picker UIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_user_message: Option<String>,
}

/// A fully-loaded session: the main chat plus every sub-agent file in the
/// same session UUID directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Session UUID from the directory name (not the inner `sessionId`).
    pub session_uuid: String,
    pub project_path: Option<String>,
    /// The main chat file (the one without `kind: "subagent"`). If the
    /// directory only contains sub-agent files, the first one is used.
    pub main: ChatFile,
    /// Sub-agent chat files found alongside the main chat.
    pub sub_agents: Vec<ChatFile>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_activity: Option<DateTime<Utc>>,
}

impl Conversation {
    pub fn new(session_uuid: String, main: ChatFile) -> Self {
        let started_at = main.start_time;
        let last_activity = main.last_updated;
        Self {
            session_uuid,
            project_path: None,
            main,
            sub_agents: Vec::new(),
            started_at,
            last_activity,
        }
    }

    /// Messages in the main chat, in file order.
    pub fn messages(&self) -> &[GeminiMessage] {
        &self.main.messages
    }

    /// Total message count across main chat and sub-agent chats.
    pub fn total_message_count(&self) -> usize {
        self.main.messages.len()
            + self
                .sub_agents
                .iter()
                .map(|s| s.messages.len())
                .sum::<usize>()
    }

    /// First user-message text from the main chat (untruncated).
    pub fn first_user_text(&self) -> Option<String> {
        self.main.messages.iter().find_map(|m| {
            if m.role == GeminiRole::User {
                let t = m.content.text();
                if t.is_empty() { None } else { Some(t) }
            } else {
                None
            }
        })
    }

    /// Text of the first user message, truncated to `max_len` characters.
    pub fn title(&self, max_len: usize) -> Option<String> {
        self.first_user_text().map(|t| {
            if t.chars().count() > max_len {
                let trunc: String = t.chars().take(max_len).collect();
                format!("{}...", trunc)
            } else {
                t
            }
        })
    }

    /// Iterate all messages (main + sub-agents) in document order.
    pub fn all_messages(&self) -> impl Iterator<Item = &GeminiMessage> {
        self.main
            .messages
            .iter()
            .chain(self.sub_agents.iter().flat_map(|s| s.messages.iter()))
    }

    /// Find the sub-agent chat whose inner `sessionId` matches.
    pub fn sub_agent_by_session_id(&self, session_id: &str) -> Option<&ChatFile> {
        self.sub_agents.iter().find(|s| s.session_id == session_id)
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_CHAT: &str = r#"{
  "sessionId": "abc",
  "projectHash": "deadbeef",
  "startTime": "2026-04-17T15:23:55.515Z",
  "lastUpdated": "2026-04-17T15:27:05.630Z",
  "messages": [
    {"id":"m1","timestamp":"2026-04-17T15:23:55.515Z","type":"user","content":[{"text":"Hello"}]},
    {"id":"m2","timestamp":"2026-04-17T15:23:57.196Z","type":"gemini","content":"Hi","model":"gemini-3-flash-preview","tokens":{"input":10,"output":2,"cached":0,"thoughts":0,"tool":0,"total":12}}
  ]
}"#;

    #[test]
    fn test_parse_minimal_chat() {
        let chat: ChatFile = serde_json::from_str(MIN_CHAT).unwrap();
        assert_eq!(chat.session_id, "abc");
        assert_eq!(chat.project_hash, "deadbeef");
        assert_eq!(chat.messages.len(), 2);
        assert_eq!(chat.messages[0].role, GeminiRole::User);
        assert_eq!(chat.messages[1].role, GeminiRole::Gemini);
        assert_eq!(chat.messages[1].content.text(), "Hi");
        assert_eq!(
            chat.messages[1].model.as_deref(),
            Some("gemini-3-flash-preview")
        );
        assert_eq!(chat.messages[1].tokens.as_ref().unwrap().input, Some(10));
    }

    #[test]
    fn test_content_text_from_parts() {
        let c = GeminiContent::Parts(vec![
            TextPart {
                text: Some("a".into()),
                extra: Default::default(),
            },
            TextPart {
                text: None,
                extra: Default::default(),
            },
            TextPart {
                text: Some("b".into()),
                extra: Default::default(),
            },
        ]);
        assert_eq!(c.text(), "a\nb");
    }

    #[test]
    fn test_content_text_empty_string() {
        let c = GeminiContent::Text(String::new());
        assert!(c.is_empty());
        assert_eq!(c.text(), "");
    }

    #[test]
    fn test_parse_tool_call_with_result() {
        let json = r#"{
  "id":"t1","name":"read_file","args":{"path":"x"},
  "status":"success","timestamp":"2026-04-17T15:23:57Z",
  "result":[{"functionResponse":{"id":"t1","name":"read_file","response":{"output":"hello"}}}]
}"#;
        let tc: ToolCall = serde_json::from_str(json).unwrap();
        assert_eq!(tc.name, "read_file");
        assert_eq!(tc.status, "success");
        assert!(!tc.is_error());
        assert_eq!(tc.result_text(), "hello");
    }

    #[test]
    fn test_parse_tool_call_error() {
        let json = r#"{"id":"t1","name":"run_shell_command","args":{},"status":"error","timestamp":"2026-04-17T15:23:57Z"}"#;
        let tc: ToolCall = serde_json::from_str(json).unwrap();
        assert!(tc.is_error());
    }

    #[test]
    fn test_parse_subagent_file() {
        let json = r#"{
  "sessionId":"qclszz",
  "projectHash":"d",
  "kind":"subagent",
  "summary":"found the bug",
  "messages":[]
}"#;
        let chat: ChatFile = serde_json::from_str(json).unwrap();
        assert_eq!(chat.kind.as_deref(), Some("subagent"));
        assert_eq!(chat.summary.as_deref(), Some("found the bug"));
    }

    #[test]
    fn test_conversation_helpers() {
        let main: ChatFile = serde_json::from_str(MIN_CHAT).unwrap();
        let convo = Conversation::new("session-uuid".to_string(), main);
        assert_eq!(convo.total_message_count(), 2);
        assert_eq!(convo.first_user_text().as_deref(), Some("Hello"));
        assert_eq!(convo.title(3).as_deref(), Some("Hel..."));
        assert_eq!(convo.title(100).as_deref(), Some("Hello"));
    }

    #[test]
    fn test_thoughts_optional() {
        let json = r#"{"id":"m","timestamp":"t","type":"gemini","content":"","thoughts":[{"subject":"s","description":"d","timestamp":"t"}]}"#;
        let msg: GeminiMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.thoughts().len(), 1);
        assert_eq!(msg.thoughts()[0].subject.as_deref(), Some("s"));
    }

    #[test]
    fn test_log_entry() {
        let json =
            r#"{"sessionId":"s","messageId":0,"type":"user","message":"hi","timestamp":"t"}"#;
        let e: LogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.session_id, "s");
        assert_eq!(e.message, "hi");
    }

    #[test]
    fn test_function_response_nested() {
        let json = r#"[{"functionResponse":{"id":"t","name":"x","response":"plain"}}]"#;
        let responses: Vec<FunctionResponse> = serde_json::from_str(json).unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].function_response.name, "x");
    }

    #[test]
    fn test_response_to_text_fallback() {
        assert_eq!(response_to_text(&serde_json::json!("hello")), "hello");
        assert_eq!(response_to_text(&serde_json::json!(null)), "");
        assert_eq!(response_to_text(&serde_json::json!({"output":"ok"})), "ok");
        // Non-string, non-object values stringify
        assert_eq!(response_to_text(&serde_json::json!(42)), "42");
    }

    // ── Accessors for Option<Vec<T>> fields ──────────────────────────

    #[test]
    fn test_accessors_return_empty_slice_when_absent() {
        let msg: GeminiMessage =
            serde_json::from_str(r#"{"id":"m","timestamp":"ts","type":"user","content":""}"#)
                .unwrap();
        assert!(msg.thoughts.is_none());
        assert!(msg.tool_calls.is_none());
        assert!(msg.thoughts().is_empty());
        assert!(msg.tool_calls().is_empty());

        let chat: ChatFile =
            serde_json::from_str(r#"{"sessionId":"s","projectHash":"","messages":[]}"#).unwrap();
        assert!(chat.directories.is_none());
        assert!(chat.directories().is_empty());
    }

    #[test]
    fn test_accessors_return_slice_when_empty_array() {
        let msg: GeminiMessage = serde_json::from_str(
            r#"{"id":"m","timestamp":"ts","type":"user","content":"","thoughts":[],"toolCalls":[]}"#,
        )
        .unwrap();
        assert!(msg.thoughts.is_some());
        assert!(msg.tool_calls.is_some());
        assert!(msg.thoughts().is_empty());
        assert!(msg.tool_calls().is_empty());
    }

    // ── GeminiRole exhaustive round-trip ────────────────────────────

    #[test]
    fn test_gemini_role_all_variants_roundtrip() {
        for (json, role) in [
            (r#""user""#, GeminiRole::User),
            (r#""gemini""#, GeminiRole::Gemini),
            (r#""info""#, GeminiRole::Info),
        ] {
            let parsed: GeminiRole = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, role);
            let back = serde_json::to_string(&role).unwrap();
            assert_eq!(back, json);
        }
        // Unknown values survive via Other(String)
        let parsed: GeminiRole = serde_json::from_str(r#""plan""#).unwrap();
        assert_eq!(parsed, GeminiRole::Other("plan".to_string()));
        assert_eq!(parsed.as_str(), "plan");
        let back = serde_json::to_string(&parsed).unwrap();
        assert_eq!(back, r#""plan""#);
    }

    #[test]
    fn test_gemini_role_as_str() {
        assert_eq!(GeminiRole::User.as_str(), "user");
        assert_eq!(GeminiRole::Gemini.as_str(), "gemini");
        assert_eq!(GeminiRole::Info.as_str(), "info");
        assert_eq!(GeminiRole::Other("x".into()).as_str(), "x");
    }

    // ── ToolCall::file_diff / result_display_text ────────────────────

    #[test]
    fn test_tool_call_file_diff_from_dict() {
        let tc: ToolCall = serde_json::from_str(
            r#"{"id":"t","name":"write_file","args":{},"status":"success","timestamp":"ts","resultDisplay":{"fileDiff":"@@ -0,0 +1 @@\n+x"}}"#,
        )
        .unwrap();
        assert_eq!(tc.file_diff().as_deref(), Some("@@ -0,0 +1 @@\n+x"));
    }

    #[test]
    fn test_tool_call_file_diff_absent_when_no_result_display() {
        let tc: ToolCall = serde_json::from_str(
            r#"{"id":"t","name":"write_file","args":{},"status":"success","timestamp":"ts"}"#,
        )
        .unwrap();
        assert!(tc.file_diff().is_none());
    }

    #[test]
    fn test_tool_call_file_diff_absent_when_plain_string() {
        let tc: ToolCall = serde_json::from_str(
            r#"{"id":"t","name":"run_shell_command","args":{},"status":"success","timestamp":"ts","resultDisplay":"output text"}"#,
        )
        .unwrap();
        assert!(tc.file_diff().is_none());
        assert_eq!(tc.result_display_text().as_deref(), Some("output text"));
    }

    #[test]
    fn test_tool_call_result_display_text_absent_when_dict() {
        let tc: ToolCall = serde_json::from_str(
            r#"{"id":"t","name":"write_file","args":{},"status":"success","timestamp":"ts","resultDisplay":{"fileDiff":"x"}}"#,
        )
        .unwrap();
        assert!(tc.result_display_text().is_none());
    }

    // ── Conversation::sub_agent_by_session_id, all_messages ──────────

    #[test]
    fn test_sub_agent_by_session_id() {
        let main: ChatFile =
            serde_json::from_str(r#"{"sessionId":"main","projectHash":"","messages":[]}"#).unwrap();
        let mut convo = Conversation::new("uuid".into(), main);
        let sub: ChatFile = serde_json::from_str(
            r#"{"sessionId":"helper","projectHash":"","kind":"subagent","messages":[]}"#,
        )
        .unwrap();
        convo.sub_agents.push(sub);
        assert!(convo.sub_agent_by_session_id("helper").is_some());
        assert!(convo.sub_agent_by_session_id("nope").is_none());
    }

    #[test]
    fn test_conversation_all_messages_covers_main_and_subs() {
        let main: ChatFile = serde_json::from_str(
            r#"{"sessionId":"m","projectHash":"","messages":[
  {"id":"u","timestamp":"ts","type":"user","content":"hi"}
]}"#,
        )
        .unwrap();
        let mut convo = Conversation::new("u".into(), main);
        let sub: ChatFile = serde_json::from_str(
            r#"{"sessionId":"s","projectHash":"","kind":"subagent","messages":[
  {"id":"s1","timestamp":"ts","type":"user","content":"sub"}
]}"#,
        )
        .unwrap();
        convo.sub_agents.push(sub);
        let all: Vec<&GeminiMessage> = convo.all_messages().collect();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_conversation_messages_accessor() {
        let main: ChatFile = serde_json::from_str(
            r#"{"sessionId":"m","projectHash":"","messages":[
  {"id":"u","timestamp":"ts","type":"user","content":"hi"}
]}"#,
        )
        .unwrap();
        let convo = Conversation::new("u".into(), main);
        assert_eq!(convo.messages().len(), 1);
    }

    // ── GeminiContent further coverage ─────────────────────────────

    #[test]
    fn test_content_default_is_empty_text() {
        let c = GeminiContent::default();
        assert!(matches!(c, GeminiContent::Text(ref s) if s.is_empty()));
    }

    #[test]
    fn test_content_parts_is_empty_all_none_texts() {
        let c = GeminiContent::Parts(vec![TextPart {
            text: None,
            extra: Default::default(),
        }]);
        assert!(c.is_empty());
    }
}
