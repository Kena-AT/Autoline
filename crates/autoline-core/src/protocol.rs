use crate::history::{HistoryKind, ShellKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuggestionSource {
    Trie,
    History,
    Ngram,
    Fuzzy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuggestRequest {
    pub session_id: String,
    pub shell: ShellKind,
    pub line: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestResponse {
    pub suggestion: Option<String>,
    pub source: SuggestionSource,
    pub confidence: f32,
}

impl Default for SuggestResponse {
    fn default() -> Self {
        Self {
            suggestion: None,
            source: SuggestionSource::Trie,
            confidence: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordCommandPayload {
    pub line: String,
    pub kind: HistoryKind,
    pub shell: ShellKind,
    pub cwd: String,
    pub tool: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonStatusResponse {
    pub pid: u32,
    pub uptime_secs: u64,
    pub history_count: u64,
    pub models_active: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Request {
    Suggest(SuggestRequest),
    RecordCommand(RecordCommandPayload),
    Ping,
    Status,
    ReloadModel,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Response {
    Suggestion(SuggestResponse),
    Status(DaemonStatusResponse),
    Ack,
    Pong,
    Error(String),
}

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub async fn read_framed_msg<T, R>(reader: &mut R) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await?;
    let val: T = rmp_serde::from_slice(&payload)?;
    Ok(val)
}

pub async fn write_framed_msg<T, W>(writer: &mut W, val: &T) -> anyhow::Result<()>
where
    T: Serialize,
    W: AsyncWrite + Unpin,
{
    let payload = rmp_serde::to_vec_named(val)?;
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_msgpack<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(
        value: &T,
    ) {
        let bytes = rmp_serde::to_vec_named(value).expect("serialize");
        let back: T = rmp_serde::from_slice(&bytes).expect("deserialize");
        assert_eq!(*value, back);
    }

    #[test]
    fn suggest_request_roundtrip() {
        let req = SuggestRequest {
            session_id: "sess-1".to_string(),
            shell: ShellKind::Zsh,
            line: "git co".to_string(),
            cwd: "/home/user".to_string(),
        };
        let bytes = rmp_serde::to_vec_named(&req).unwrap();
        let back: SuggestRequest = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.line, "git co");
    }

    #[test]
    fn suggest_response_some_roundtrip() {
        let resp = SuggestResponse {
            suggestion: Some("checkout main".to_string()),
            source: SuggestionSource::Trie,
            confidence: 0.95,
        };
        let bytes = rmp_serde::to_vec_named(&resp).unwrap();
        let back: SuggestResponse = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back.suggestion.as_deref(), Some("checkout main"));
        assert_eq!(back.confidence, 0.95);
    }

    #[test]
    fn suggest_response_none_roundtrip() {
        let resp = SuggestResponse::default();
        assert!(resp.suggestion.is_none());
        let bytes = rmp_serde::to_vec_named(&resp).unwrap();
        let back: SuggestResponse = rmp_serde::from_slice(&bytes).unwrap();
        assert!(back.suggestion.is_none());
    }

    #[test]
    fn record_payload_roundtrip() {
        let payload = RecordCommandPayload {
            line: "git status".to_string(),
            kind: HistoryKind::Command,
            shell: ShellKind::Bash,
            cwd: "/repo".to_string(),
            tool: None,
        };
        roundtrip_msgpack(&Request::RecordCommand(payload));
    }

    #[test]
    fn request_enum_roundtrip() {
        roundtrip_msgpack(&Request::Ping);
        roundtrip_msgpack(&Request::Status);
        roundtrip_msgpack(&Request::Shutdown);
        roundtrip_msgpack(&Request::ReloadModel);

        let req = Request::Suggest(SuggestRequest {
            session_id: "x".into(),
            shell: ShellKind::Cmd,
            line: "cargo b".into(),
            cwd: "/p".into(),
        });
        let bytes = rmp_serde::to_vec_named(&req).unwrap();
        let back: Request = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(back, Request::Suggest(_)));
    }

    #[test]
    fn response_enum_roundtrip() {
        roundtrip_msgpack(&Response::Ack);
        roundtrip_msgpack(&Response::Pong);
        roundtrip_msgpack(&Response::Status(DaemonStatusResponse {
            pid: 1234,
            uptime_secs: 42,
            history_count: 100,
            models_active: true,
            version: "0.1.0".to_string(),
        }));
        roundtrip_msgpack(&Response::Error("oops".to_string()));
    }

    #[test]
    fn json_roundtrip_also_works() {
        let resp = SuggestResponse {
            suggestion: Some("hello".to_string()),
            source: SuggestionSource::History,
            confidence: 0.75,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: SuggestResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, SuggestionSource::History);
    }
}
