//! The JSON Claude Code pipes to a hook on stdin. Only the fields Air reads; everything else
//! is ignored so new fields never break parsing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookEvent {
    SessionStart,
    SessionEnd,
    PreToolUse,
    PostToolUse,
    PermissionRequest,
    /// A tool call was refused by a rule, a hook, or the human (observation only; the
    /// friction Air did not cause, plan 0006 C4).
    PermissionDenied,
    /// A tool ran and failed (observation only).
    PostToolUseFailure,
    PreCompact,
    Stop,
    SubagentStop,
    Notification,
    UserPromptSubmit,
    #[serde(other)]
    Other,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            HookEvent::SessionStart => "SessionStart",
            HookEvent::SessionEnd => "SessionEnd",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::PermissionRequest => "PermissionRequest",
            HookEvent::PermissionDenied => "PermissionDenied",
            HookEvent::PostToolUseFailure => "PostToolUseFailure",
            HookEvent::PreCompact => "PreCompact",
            HookEvent::Stop => "Stop",
            HookEvent::SubagentStop => "SubagentStop",
            HookEvent::Notification => "Notification",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HookInput {
    pub session_id: String,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    #[serde(alias = "hookEventName")]
    pub hook_event_name: Option<HookEvent>,
    #[serde(alias = "toolName")]
    pub tool_name: Option<String>,
    #[serde(alias = "toolInput")]
    pub tool_input: Option<serde_json::Value>,
    /// PostToolUseFailure: the tool's error text.
    pub error: Option<String>,
    /// Stop / SubagentStop: true when this stop was itself caused by a Stop hook — the loop
    /// guard (never block again when set).
    pub stop_hook_active: Option<bool>,
    /// PreCompact: "manual" | "auto".
    pub trigger: Option<String>,
    /// SessionEnd reason.
    pub reason: Option<String>,
    pub permission_mode: Option<String>,
}

impl HookInput {
    pub fn parse(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    pub fn event(&self) -> HookEvent {
        self.hook_event_name.unwrap_or(HookEvent::Other)
    }

    /// For Edit/Write/MultiEdit tool calls: the target path, if present.
    pub fn edited_path(&self) -> Option<String> {
        let name = self.tool_name.as_deref()?;
        if !matches!(name, "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
            return None;
        }
        self.tool_input
            .as_ref()?
            .get("file_path")
            .or_else(|| self.tool_input.as_ref()?.get("notebook_path"))?
            .as_str()
            .map(str::to_string)
    }

    /// For `SendMessage` tool calls: the peer being addressed (air-0lk).
    pub fn send_message_to(&self) -> Option<&str> {
        if self.tool_name.as_deref() != Some("SendMessage") {
            return None;
        }
        self.tool_input.as_ref()?.get("to")?.as_str()
    }

    /// For Bash tool calls: the command string.
    pub fn bash_command(&self) -> Option<&str> {
        if self.tool_name.as_deref() != Some("Bash") {
            return None;
        }
        self.tool_input.as_ref()?.get("command")?.as_str()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_post_tool_use_edit() {
        let j = r#"{"session_id":"s1","transcript_path":"/t.jsonl","cwd":"/repo",
            "hook_event_name":"PostToolUse","tool_name":"Edit",
            "tool_input":{"file_path":"/repo/src/a.rs","old_string":"x","new_string":"y"},
            "tool_response":{"ok":true},"future_field":1}"#;
        let h = HookInput::parse(j).unwrap();
        assert_eq!(h.event(), HookEvent::PostToolUse);
        assert_eq!(h.edited_path().as_deref(), Some("/repo/src/a.rs"));
        assert!(h.bash_command().is_none());
    }

    #[test]
    fn unknown_event_is_other_and_stop_guard_reads() {
        let h = HookInput::parse(r#"{"session_id":"s","hook_event_name":"Brand New"}"#).unwrap();
        assert_eq!(h.event(), HookEvent::Other);
        let s = HookInput::parse(
            r#"{"session_id":"s","hook_event_name":"Stop","stop_hook_active":true}"#,
        )
        .unwrap();
        assert_eq!(s.stop_hook_active, Some(true));
    }

    #[test]
    fn bash_command_extracts() {
        let h = HookInput::parse(
            r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"bd close fd-1"}}"#,
        )
        .unwrap();
        assert_eq!(h.bash_command(), Some("bd close fd-1"));
    }
}
