use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub name: String,
    pub audio_path: PathBuf,
    pub transcript_text: Option<String>,
    pub protocol_text: Option<String>,
    pub created_at: i64,
}

impl Meeting {
    pub fn new(name: String, audio_path: PathBuf) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            audio_path,
            transcript_text: None,
            protocol_text: None,
            created_at: now_unix(),
        }
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock went before epoch")
        .as_secs() as i64
}
