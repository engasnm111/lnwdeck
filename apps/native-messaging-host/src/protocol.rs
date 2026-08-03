use serde::{Deserialize, Serialize};

pub const MAX_MESSAGE_SIZE: usize = 1_048_576; // 1 MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub version: u32,
    pub timestamp: String,
    pub nonce: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

pub fn read_message(reader: &mut impl std::io::Read) -> Result<QuotaMessage, ProtocolError> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .map_err(|_| ProtocolError::ReadError)?;

    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::Oversized(len));
    }

    let mut msg_buf = vec![0u8; len];
    reader
        .read_exact(&mut msg_buf)
        .map_err(|_| ProtocolError::ReadError)?;

    let msg: QuotaMessage =
        serde_json::from_slice(&msg_buf).map_err(|e| ProtocolError::ParseError(e.to_string()))?;

    Ok(msg)
}

pub fn write_message(
    writer: &mut impl std::io::Write,
    msg: &QuotaMessage,
) -> Result<(), ProtocolError> {
    let json = serde_json::to_vec(msg).map_err(|e| ProtocolError::SerializeError(e.to_string()))?;

    if json.len() > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::Oversized(json.len()));
    }

    let len = json.len() as u32;
    writer
        .write_all(&len.to_le_bytes())
        .map_err(|_| ProtocolError::WriteError)?;
    writer
        .write_all(&json)
        .map_err(|_| ProtocolError::WriteError)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("read error")]
    ReadError,
    #[error("write error")]
    WriteError,
    #[error("message too large: {0} bytes")]
    Oversized(usize),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("serialize error: {0}")]
    SerializeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip() {
        let msg = QuotaMessage {
            msg_type: "health_check".to_string(),
            version: 1,
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            nonce: "test1234".to_string(),
            payload: Some(serde_json::json!({"status": "ok"})),
        };

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let decoded = read_message(&mut cursor).unwrap();

        assert_eq!(decoded.msg_type, msg.msg_type);
        assert_eq!(decoded.version, msg.version);
        assert_eq!(decoded.nonce, msg.nonce);
    }
}
