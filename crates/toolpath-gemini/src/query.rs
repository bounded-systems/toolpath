//! Query/filter operations over a loaded `Conversation`.

use crate::types::{Conversation, GeminiMessage, GeminiRole};
use chrono::{DateTime, Utc};

pub struct ConversationQuery<'a> {
    conversation: &'a Conversation,
}

impl<'a> ConversationQuery<'a> {
    pub fn new(conversation: &'a Conversation) -> Self {
        Self { conversation }
    }

    /// Messages with the given role.
    pub fn by_role(&self, role: GeminiRole) -> Vec<&'a GeminiMessage> {
        self.conversation
            .all_messages()
            .filter(|m| m.role == role)
            .collect()
    }

    /// Messages whose parsed timestamp falls in `[start, end]`.
    pub fn by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&'a GeminiMessage> {
        self.conversation
            .all_messages()
            .filter(|m| {
                if let Ok(ts) = m.timestamp.parse::<DateTime<Utc>>() {
                    ts >= start && ts <= end
                } else {
                    false
                }
            })
            .collect()
    }

    /// Messages that invoke a tool with the given `name`.
    pub fn tool_uses_by_name(&self, tool_name: &str) -> Vec<&'a GeminiMessage> {
        self.conversation
            .all_messages()
            .filter(|m| m.tool_calls().iter().any(|t| t.name == tool_name))
            .collect()
    }

    /// Messages whose text or tool-call result text contains `search`
    /// (case-insensitive).
    pub fn contains_text(&self, search: &str) -> Vec<&'a GeminiMessage> {
        let needle = search.to_lowercase();
        self.conversation
            .all_messages()
            .filter(|m| {
                if m.content.text().to_lowercase().contains(&needle) {
                    return true;
                }
                m.tool_calls()
                    .iter()
                    .any(|t| t.result_text().to_lowercase().contains(&needle))
            })
            .collect()
    }

    /// Messages where at least one tool call reported an error.
    pub fn errors(&self) -> Vec<&'a GeminiMessage> {
        self.conversation
            .all_messages()
            .filter(|m| m.tool_calls().iter().any(|t| t.is_error()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChatFile, Conversation, GeminiContent, GeminiMessage, ToolCall};

    fn msg(role: GeminiRole, ts: &str, text: &str) -> GeminiMessage {
        GeminiMessage {
            id: ts.into(),
            timestamp: ts.into(),
            role,
            content: GeminiContent::Text(text.into()),
            thoughts: None,
            tokens: None,
            model: None,
            tool_calls: None,
            extra: Default::default(),
        }
    }

    fn tool_msg(role: GeminiRole, ts: &str, tool_name: &str, error: bool) -> GeminiMessage {
        let mut m = msg(role, ts, "");
        m.tool_calls = Some(vec![ToolCall {
            id: "t".into(),
            name: tool_name.into(),
            args: serde_json::Value::Null,
            status: if error {
                "error".into()
            } else {
                "success".into()
            },
            timestamp: ts.into(),
            result: vec![],
            result_display: None,
            description: None,
            display_name: None,
            extra: Default::default(),
        }]);
        m
    }

    fn build_convo(messages: Vec<GeminiMessage>) -> Conversation {
        let chat = ChatFile {
            session_id: "s".into(),
            project_hash: "h".into(),
            start_time: None,
            last_updated: None,
            directories: None,
            kind: None,
            summary: None,
            messages,
            extra: Default::default(),
        };
        Conversation::new("session-uuid".into(), chat)
    }

    #[test]
    fn test_by_role() {
        let convo = build_convo(vec![
            msg(GeminiRole::User, "2026-04-17T10:00:00Z", "Hi"),
            msg(GeminiRole::Gemini, "2026-04-17T10:00:01Z", "Hey"),
        ]);
        let q = ConversationQuery::new(&convo);
        assert_eq!(q.by_role(GeminiRole::User).len(), 1);
        assert_eq!(q.by_role(GeminiRole::Gemini).len(), 1);
    }

    #[test]
    fn test_tool_uses_by_name() {
        let convo = build_convo(vec![
            msg(GeminiRole::User, "2026-04-17T10:00:00Z", "read"),
            tool_msg(
                GeminiRole::Gemini,
                "2026-04-17T10:00:01Z",
                "read_file",
                false,
            ),
            tool_msg(
                GeminiRole::Gemini,
                "2026-04-17T10:00:02Z",
                "write_file",
                false,
            ),
        ]);
        let q = ConversationQuery::new(&convo);
        assert_eq!(q.tool_uses_by_name("read_file").len(), 1);
        assert_eq!(q.tool_uses_by_name("missing").len(), 0);
    }

    #[test]
    fn test_contains_text_case_insensitive() {
        let convo = build_convo(vec![msg(
            GeminiRole::User,
            "2026-04-17T10:00:00Z",
            "Look at AUTH.rs",
        )]);
        let q = ConversationQuery::new(&convo);
        assert_eq!(q.contains_text("auth").len(), 1);
        assert_eq!(q.contains_text("db").len(), 0);
    }

    #[test]
    fn test_errors() {
        let convo = build_convo(vec![
            tool_msg(
                GeminiRole::Gemini,
                "2026-04-17T10:00:00Z",
                "run_shell_command",
                false,
            ),
            tool_msg(
                GeminiRole::Gemini,
                "2026-04-17T10:00:01Z",
                "run_shell_command",
                true,
            ),
        ]);
        let q = ConversationQuery::new(&convo);
        let errs = q.errors();
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn test_by_time_range() {
        let convo = build_convo(vec![
            msg(GeminiRole::User, "2026-04-17T10:00:00Z", "early"),
            msg(GeminiRole::User, "2026-04-17T12:00:00Z", "mid"),
            msg(GeminiRole::User, "2026-04-17T14:00:00Z", "late"),
        ]);
        let q = ConversationQuery::new(&convo);
        let start: DateTime<Utc> = "2026-04-17T11:00:00Z".parse().unwrap();
        let end: DateTime<Utc> = "2026-04-17T13:00:00Z".parse().unwrap();
        let hits = q.by_time_range(start, end);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].content.text(), "mid");
    }

    #[test]
    fn test_contains_text_matches_tool_result() {
        let mut m = tool_msg(GeminiRole::Gemini, "ts", "read_file", false);
        if let Some(calls) = m.tool_calls.as_mut() {
            calls[0].result = vec![crate::types::FunctionResponse {
                function_response: crate::types::FunctionResponseBody {
                    id: "t".into(),
                    name: "read_file".into(),
                    response: serde_json::json!({"output": "hello AUTH module"}),
                },
            }];
        }
        let convo = build_convo(vec![m]);
        let q = ConversationQuery::new(&convo);
        assert_eq!(q.contains_text("auth").len(), 1);
    }

    #[test]
    fn test_spans_main_and_sub_agents() {
        let main_chat = ChatFile {
            session_id: "main".into(),
            project_hash: "".into(),
            start_time: None,
            last_updated: None,
            directories: None,
            kind: None,
            summary: None,
            messages: vec![msg(GeminiRole::User, "ts1", "main text")],
            extra: Default::default(),
        };
        let sub_chat = ChatFile {
            session_id: "sub".into(),
            project_hash: "".into(),
            start_time: None,
            last_updated: None,
            directories: None,
            kind: Some("subagent".into()),
            summary: None,
            messages: vec![msg(GeminiRole::User, "ts2", "sub text")],
            extra: Default::default(),
        };
        let mut convo = Conversation::new("uuid".into(), main_chat);
        convo.sub_agents.push(sub_chat);
        let q = ConversationQuery::new(&convo);
        assert_eq!(q.contains_text("sub text").len(), 1);
        assert_eq!(q.contains_text("main text").len(), 1);
        assert_eq!(q.by_role(GeminiRole::User).len(), 2);
    }
}
